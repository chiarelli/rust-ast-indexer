use std::sync::mpsc::{self};
use std::sync::Arc;

use crate::app::bootstrap::ApplicationContext;
use crate::domain::normalize::normalize_import;
use crate::domain::types::{Chunk, FileRecord};
use crate::infra::parser_pool::ParserPool;
use crate::infra::walker::{walk_path, ScanOptions, WalkerError};
use rayon::prelude::*;

/// Detect language from file extension
fn detect_language(path: &str) -> Option<String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str());

    match ext {
        Some("rs") => Some("rust".to_string()),
        Some("ts") | Some("tsx") => Some("typescript".to_string()),
        Some("js") | Some("jsx") => Some("javascript".to_string()),
        Some("java") => Some("java".to_string()),
        Some(_) => None,
        None => None,
    }
}

pub struct IndexOptions {
    pub max_concurrency: usize,
    pub explicit_files: Option<Vec<String>>,
    pub extract_imports: bool,
    pub extract_calls: bool,
}

pub struct Indexer {
    ctx: Option<Arc<ApplicationContext>>,
}

pub struct IndexResult {
    pub chunks: Vec<Chunk>,
    pub files: Vec<FileRecord>,
}

impl Default for Indexer {
    fn default() -> Self {
        Self::new()
    }
}

impl Indexer {
    pub fn new() -> Self {
        Indexer { ctx: None }
    }

    pub fn from_context(ctx: Arc<ApplicationContext>) -> Self {
        Indexer { ctx: Some(ctx) }
    }

    /// Simple serial index_path (keeps previous behavior)
    pub fn index_path(&self, path: &str, opts: IndexOptions) -> Result<IndexResult, WalkerError> {
        let files = if let Some(explicit) = &opts.explicit_files {
            explicit
                .iter()
                .cloned()
                .map(|f_path| FileRecord {
                    path: f_path,
                    size: 0,
                    mtime: 0,
                    hash: "".to_string(),
                    language: None,
                })
                .collect()
        } else {
            let scan_opts = ScanOptions::new(path);
            walk_path(&scan_opts)?
        };

        let mut chunks = Vec::new();

        for (idx, file) in files.iter().enumerate() {
            let chunk = Chunk {
                id: format!("chunk-{}", idx),
                file_path: file.path.clone(),
                start_line: 1,
                end_line: 1,
                content: String::new(),
                text: String::new(),
                md5: file.hash.clone(),
                size: file.size as usize,
                language: file.language.clone(),
                symbol_id: None,
                symbol_ids: vec![],
                metadata: None,
                chunk_kind: Some("FullFile".into()),
            };

            // Emit chunk_emitted event for caller (serial path)
            let payload = crate::application::protocol::ChunkEventPayload::from(chunk.clone());
            crate::infra::jsonl::write_chunk_event(None, &payload);

            chunks.push(chunk);
        }

        Ok(IndexResult { chunks, files })
    }

    /// New: parallel indexing scaffold using Rayon and a ParserPool placeholder.
    pub fn index_path_parallel(
        &self,
        path: &str,
        opts: IndexOptions,
        job_id: Option<String>,
    ) -> Result<IndexResult, WalkerError> {
        let files = if let Some(explicit) = &opts.explicit_files {
            explicit
                .iter()
                .cloned()
                .map(|f_path| FileRecord {
                    path: f_path,
                    size: 0,
                    mtime: 0,
                    hash: "".to_string(),
                    language: None,
                })
                .collect()
        } else {
            let scan_opts = ScanOptions::new(path);
            walk_path(&scan_opts)?
        };

        let pool = if let Some(ctx) = &self.ctx {
            ctx.parser_pool.clone()
        } else {
            Arc::new(ParserPool::new())
        };

        let (tx, rx) = mpsc::channel();

        let base_path = std::sync::Arc::new(std::path::PathBuf::from(path));

        // Use Rayon thread pool to process files in parallel
        files
            .par_iter()
            .enumerate()
            .with_max_len(1)
            .for_each_with(tx.clone(), |s, (idx, file)| {
                // Determine language for this file and get adapter from pool
                let lang = detect_language(&file.path);
                let adapter_opt = lang.as_ref().and_then(|l| pool.get(l));

                // Build full path to file by joining base path and relative file path
                let full_path = base_path.join(&file.path);

                // Read and parse file content
                let text = std::fs::read_to_string(&full_path).unwrap_or_default();
                let (parsed_text, syms) = match &adapter_opt {
                    Some(adapter) => match adapter.parse_source(&text) {
                        Ok(parsed) => {
                            let syms = adapter.extract_symbols(&parsed).ok();

                            // Extract and emit import edges
                            if opts.extract_imports {
                                if let Ok(imports) = adapter.extract_imports(&parsed) {
                                    let lang_for_norm = lang.clone().unwrap_or_default();
                                    for raw_edge in imports {
                                        let normalized =
                                            normalize_import(&raw_edge, &lang_for_norm);
                                        crate::infra::jsonl::write_import_event(
                                            job_id.clone(),
                                            &normalized,
                                        );
                                    }
                                }
                            }

                            // Extract and emit call edges
                            if opts.extract_calls {
                                if let Ok(calls) = adapter.extract_calls(&parsed) {
                                    for edge in calls {
                                        crate::infra::jsonl::write_call_event(
                                            job_id.clone(),
                                            &edge,
                                        );
                                    }
                                }
                            }

                            (parsed.source.clone(), syms)
                        }
                        Err(_) => (text.clone(), None),
                    },
                    None => (text.clone(), None),
                };

                // For now, generate a simple chunk using the file content's first line
                let first_line = parsed_text.lines().next().unwrap_or("").to_string();

                let chunk = Chunk {
                    id: format!("chunk-{}", idx),
                    file_path: file.path.clone(),
                    start_line: 1,
                    end_line: 1,
                    content: first_line.clone(),
                    text: first_line.clone(),
                    md5: file.hash.clone(),
                    size: file.size as usize,
                    language: file.language.clone().or(lang),
                    symbol_id: syms.as_ref().and_then(|s| s.first().map(|s| s.id.clone())),
                    symbol_ids: syms.as_ref().map(|s| s.iter().map(|sym| sym.id.clone()).collect()).unwrap_or_default(),
                    chunk_kind: Some("FullFile".into()),
                    metadata: None,
                };

                // If no adapter for this language, still emit chunk
                if adapter_opt.is_none() && file.language.is_none() {
                    // no adapter for this language
                }

                // Send chunk to collector
                let _ = s.send((chunk.clone(), file.clone()));
                // Emit chunk_emitted event for caller
                let payload = crate::application::protocol::ChunkEventPayload::from(chunk.clone());
                crate::infra::jsonl::write_chunk_event(job_id.clone(), &payload);
            });

        drop(tx);

        // Collect results
        let mut chunks = Vec::new();
        let mut files_out = Vec::new();
        for (chunk, file) in rx {
            chunks.push(chunk);
            files_out.push(file);
        }

        // Keep deterministic order by sorting
        chunks.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        files_out.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(IndexResult {
            chunks,
            files: files_out,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn index_path_returns_files_and_chunks() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("lib.rs");
        std::fs::write(&file_path, b"fn main() {}\n").unwrap();

        let indexer = Indexer::new();
        let opts = IndexOptions {
            max_concurrency: 1,
            explicit_files: None,
            extract_imports: false,
            extract_calls: false,
        };
        let result = indexer
            .index_path(dir.path().to_str().unwrap(), opts)
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.chunks.len(), 1);
        let file = &result.files[0];
        assert_eq!(file.path, "lib.rs");
        let chunk = &result.chunks[0];
        assert_eq!(chunk.file_path, "lib.rs");
        assert_eq!(chunk.language.as_deref(), Some("rust"));
    }

    #[test]
    fn index_path_parallel_generates_same_results() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("lib.rs");
        std::fs::write(&file_path, b"fn main() {}\n").unwrap();

        let indexer = Indexer::new();
        let opts = IndexOptions {
            max_concurrency: 2,
            explicit_files: None,
            extract_imports: false,
            extract_calls: false,
        };
        let result = indexer
            .index_path_parallel(dir.path().to_str().unwrap(), opts, None)
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.chunks.len(), 1);
        let file = &result.files[0];
        assert_eq!(file.path, "lib.rs");
        // verify that parser produced a chunk containing the first line
        let chunk = &result.chunks[0];
        assert_eq!(chunk.file_path, "lib.rs");
        assert!(chunk.text.starts_with("fn main"));
    }
}
