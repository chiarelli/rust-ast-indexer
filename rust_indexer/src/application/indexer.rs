use std::sync::mpsc;
use std::sync::Arc;

use rayon::prelude::*;
use std::collections::HashMap;

use crate::app::bootstrap::ApplicationContext;
use crate::application::chunking::{ChunkStrategy, ContextInjectionChunker, SymbolBoundaryChunker};
use crate::domain::normalize::normalize_import;
use crate::domain::types::{Chunk, FileRecord, Symbol};
use crate::infra::parser_pool::ParserPool;
use crate::infra::walker::{walk_path, ScanOptions, WalkerError};

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

    pub fn index_path(&self, path: &str, opts: IndexOptions) -> Result<IndexResult, WalkerError> {
        let files = if let Some(explicit) = &opts.explicit_files {
            explicit
                .iter()
                .cloned()
                .map(|f_path| FileRecord {
                    path: f_path,
                    size: 0,
                    mtime: 0,
                    hash: String::new(),
                    language: None,
                })
                .collect()
        } else {
            let scan_opts = ScanOptions::new(path);
            walk_path(&scan_opts)?
        };

        let mut chunks = Vec::new();

        for file in &files {
            let full_path = std::path::PathBuf::from(path).join(&file.path);
            let text = std::fs::read_to_string(&full_path).unwrap_or_default();
            let generated = chunk_file_contents(&file.path, &text, file.language.clone(), None);

            for chunk in generated {
                let payload = crate::application::protocol::ChunkEventPayload::from(chunk.clone());
                crate::infra::jsonl::write_chunk_event(None, &payload);
                chunks.push(chunk);
            }
        }

        Ok(IndexResult { chunks, files })
    }

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
                    hash: String::new(),
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
        let base_path = Arc::new(std::path::PathBuf::from(path));

        files
            .par_iter()
            .enumerate()
            .with_max_len(1)
            .for_each_with(tx.clone(), |sender, (_idx, file)| {
                let lang = detect_language(&file.path);
                let adapter_opt = lang.as_ref().and_then(|language| pool.get(language));
                let full_path = base_path.join(&file.path);
                let text = std::fs::read_to_string(&full_path).unwrap_or_default();

                let (parsed_text, symbols) = match &adapter_opt {
                    Some(adapter) => match adapter.parse_source(&text) {
                        Ok(parsed) => {
                            let symbols = adapter.extract_symbols(&parsed).ok();

                            if opts.extract_imports {
                                if let Ok(imports) = adapter.extract_imports(&parsed) {
                                    let lang_for_norm = lang.clone().unwrap_or_default();
                                    for raw_edge in imports {
                                        let normalized = normalize_import(&raw_edge, &lang_for_norm);
                                        crate::infra::jsonl::write_import_event(job_id.clone(), &normalized);
                                    }
                                }
                            }

                            if opts.extract_calls {
                                if let Ok(calls) = adapter.extract_calls(&parsed) {
                                    for edge in calls {
                                        crate::infra::jsonl::write_call_event(job_id.clone(), &edge);
                                    }
                                }
                            }

                            (parsed.source.clone(), symbols)
                        }
                        Err(_) => (text.clone(), None),
                    },
                    None => (text.clone(), None),
                };

                let chunks_for_file = chunk_file_contents(
                    &file.path,
                    &parsed_text,
                    file.language.clone().or(lang),
                    symbols.as_ref(),
                );

                for chunk in chunks_for_file {
                    let chunk_for_event = chunk.clone();
                    let _ = sender.send((chunk, file.clone()));
                    let payload = crate::application::protocol::ChunkEventPayload::from(chunk_for_event);
                    crate::infra::jsonl::write_chunk_event(job_id.clone(), &payload);
                }
            });

        drop(tx);

        let mut chunks = Vec::new();
        let mut files_out: HashMap<String, FileRecord> = HashMap::new();

        for (chunk, file) in rx {
            files_out.entry(file.path.clone()).or_insert(file);
            chunks.push(chunk);
        }

        chunks.sort_by(|a, b| {
            a.file_path
                .cmp(&b.file_path)
                .then(a.start_line.cmp(&b.start_line))
                .then(a.end_line.cmp(&b.end_line))
                .then(a.id.cmp(&b.id))
        });
        let mut files_out = files_out.into_values().collect::<Vec<_>>();
        files_out.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(IndexResult { chunks, files: files_out })
    }
}

fn chunk_file_contents(
    file_path: &str,
    source: &str,
    language: Option<String>,
    symbols: Option<&Vec<Symbol>>,
) -> Vec<Chunk> {
    let normalized_symbols = symbols.map(|items| {
        let mut filtered = items
            .iter()
            .cloned()
            .filter(is_chunk_boundary_symbol)
            .collect::<Vec<_>>();

        if filtered.iter().any(has_zero_based_lines) {
            for symbol in &mut filtered {
                symbol.start_line = symbol.start_line.saturating_add(1);
                symbol.end_line = symbol.end_line.saturating_add(1);
            }
        }

        filtered
    });

    let chunks = match normalized_symbols.as_ref() {
        Some(syms) if !syms.is_empty() => {
            let decorated = ContextInjectionChunker::new(SymbolBoundaryChunker::new(0));
            decorated.chunk_file(file_path, source, Some(syms))
        }
        _ => {
            let chunker = SymbolBoundaryChunker::new(0);
            chunker.chunk_file(file_path, source, None)
        }
    };

    chunks
        .into_iter()
        .map(|mut chunk| {
            chunk.language = language.clone().or(chunk.language.clone());
            chunk
        })
        .collect()
}

fn is_chunk_boundary_symbol(symbol: &Symbol) -> bool {
    matches!(
        symbol.kind.to_lowercase().as_str(),
        "function" | "struct" | "enum" | "trait" | "impl" | "class" | "method" | "constructor" | "mod"
    )
}

fn has_zero_based_lines(symbol: &Symbol) -> bool {
    symbol.start_line == 0 || symbol.end_line == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_file(dir: &std::path::Path, relative: &str, content: &str) {
        let file_path = dir.join(relative);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(file_path, content).unwrap();
    }

    #[test]
    fn index_path_returns_files_and_chunks() {
        let dir = tempdir().unwrap();
        write_file(dir.path(), "lib.rs", "fn main() {}\n");

        let indexer = Indexer::new();
        let opts = IndexOptions {
            max_concurrency: 1,
            explicit_files: None,
            extract_imports: false,
            extract_calls: false,
        };
        let result = indexer.index_path(dir.path().to_str().unwrap(), opts).unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.chunks.len(), 1);
        let file = &result.files[0];
        assert_eq!(file.path, "lib.rs");
        let chunk = &result.chunks[0];
        assert_eq!(chunk.file_path, "lib.rs");
        assert_eq!(chunk.language.as_deref(), Some("rust"));
        assert!(chunk.content.contains("fn main"));
    }

    #[test]
    fn index_path_parallel_falls_back_to_full_file_for_unknown_languages() {
        let dir = tempdir().unwrap();
        write_file(dir.path(), "notes.txt", "alpha\nbeta\n");

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
        let chunk = &result.chunks[0];
        assert_eq!(chunk.chunk_kind.as_deref(), Some("FullFile"));
        assert_eq!(chunk.file_path, "notes.txt");
        assert_eq!(chunk.content, "alpha\nbeta\n");
    }

    #[cfg(feature = "parsing")]
    fn build_context() -> Arc<ApplicationContext> {
        let registry = Arc::new(crate::app::bootstrap::Registry::new());
        registry.register("rust", Arc::new(crate::adapters::rust::RustAdapter));
        registry.register("typescript", Arc::new(crate::adapters::typescript::TypeScriptAdapter));
        registry.register("javascript", Arc::new(crate::adapters::typescript::TypeScriptAdapter));
        registry.register("java", Arc::new(crate::adapters::java::JavaAdapter));

        let pool = ParserPool::new();
        pool.register("rust", Arc::new(crate::adapters::rust::RustAdapter));
        pool.register("typescript", Arc::new(crate::adapters::typescript::TypeScriptAdapter));
        pool.register("javascript", Arc::new(crate::adapters::typescript::TypeScriptAdapter));
        pool.register("java", Arc::new(crate::adapters::java::JavaAdapter));

        Arc::new(ApplicationContext {
            registry,
            parser_pool: Arc::new(pool),
            config: crate::app::bootstrap::Config::new(1, 10),
            metrics: None,
            logger: None,
        })
    }

    #[cfg(feature = "parsing")]
    #[test]
    fn index_path_parallel_uses_symbol_chunks_with_context_prefix() {
        let dir = tempdir().unwrap();
        write_file(
            dir.path(),
            "src/lib.rs",
            "use std::fmt;\n\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        );

        let indexer = Indexer::from_context(build_context());
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
        assert!(!result.chunks.is_empty());

        let chunk = result
            .chunks
            .iter()
            .find(|chunk| chunk.chunk_kind.as_deref() == Some("Symbol") && chunk.content.contains("pub fn add"))
            .expect("expected a symbol chunk for pub fn add");
        assert!(chunk.symbol_ids.len() >= 1);
        assert!(chunk.content.starts_with("use std::fmt;\n\n"));
        assert_eq!(chunk.metadata.as_ref().and_then(|meta| meta.get("has_context_prefix")), Some(&serde_json::Value::Bool(true)));
        assert_eq!(chunk.language.as_deref(), Some("rust"));
    }
}
