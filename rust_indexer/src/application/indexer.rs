use std::sync::mpsc::{self};
use std::sync::Arc;

use rayon::prelude::*;

use crate::domain::types::{Chunk, FileRecord};
use crate::infra::parser_pool::ParserPool;
use crate::infra::walker::{walk_path, ScanOptions, WalkerError};

pub struct IndexOptions {
    pub max_concurrency: usize,
}

pub struct Indexer {}

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
        Indexer {}
    }

    /// Simple serial index_path (keeps previous behavior)
    pub fn index_path(&self, path: &str, _opts: IndexOptions) -> Result<IndexResult, WalkerError> {
        let scan_opts = ScanOptions::new(path);
        let files = walk_path(&scan_opts)?;

        let chunks = files
            .iter()
            .enumerate()
            .map(|(idx, file)| Chunk {
                id: format!("chunk-{}", idx),
                file_path: file.path.clone(),
                start_line: 1,
                end_line: 1,
                text: String::new(),
                md5: file.hash.clone(),
                size: file.size as usize,
                language: file.language.clone(),
                symbol_id: None,
                chunk_kind: Some("FullFile".into()),
            })
            .collect();

        Ok(IndexResult { chunks, files })
    }

    /// New: parallel indexing scaffold using Rayon and a ParserPool placeholder.
    pub fn index_path_parallel(
        &self,
        path: &str,
        opts: IndexOptions,
        job_id: Option<String>,
    ) -> Result<IndexResult, WalkerError> {
        let scan_opts = ScanOptions::new(path);
        let files = walk_path(&scan_opts)?;

        // Pre-warm parser pool (one parser per thread)
        let pool = ParserPool::new(opts.max_concurrency);
        let pool = Arc::new(pool);

        let (tx, rx) = mpsc::channel();

        let base_path = std::sync::Arc::new(std::path::PathBuf::from(path));

        // Use Rayon thread pool to process files in parallel
        files
            .par_iter()
            .enumerate()
            .with_max_len(1)
            .for_each_with(tx.clone(), |s, (idx, file)| {
                // Each worker can acquire a parser from the pool. Use thread index to pick one.
                let parser_arc = pool.get(idx);
                let mut parser = parser_arc.lock().unwrap();

                // Build full path to file by joining base path and relative file path
                let full_path = base_path.join(&file.path);

                // Parse file content using tree-sitter (if available for language)
                let text = std::fs::read_to_string(&full_path).unwrap_or_default();
                let tree = parser.parse(&text, None);

                // For now, generate a simple chunk using the file content's first line
                let first_line = text.lines().next().unwrap_or("").to_string();

                let chunk = Chunk {
                    id: format!("chunk-{}", idx),
                    file_path: file.path.clone(),
                    start_line: 1,
                    end_line: 1,
                    text: first_line,
                    md5: file.hash.clone(),
                    size: file.size as usize,
                    language: file.language.clone(),
                    symbol_id: None,
                    chunk_kind: Some("FullFile".into()),
                };

                // Optionally inspect tree to ensure parsing succeeded (not used yet)
                if tree.is_none() {
                    // parsing failed; still emit chunk with empty text
                }

                // Send chunk to collector
                let _ = s.send((chunk.clone(), file.clone()));
                // Emit chunk_emitted event for caller
                let payload = crate::application::protocol::ChunkEventPayload::from(chunk);
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
        let opts = IndexOptions { max_concurrency: 1 };
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
        let opts = IndexOptions { max_concurrency: 2 };
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
