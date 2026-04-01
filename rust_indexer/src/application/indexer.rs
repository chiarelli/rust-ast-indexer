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
    ) -> Result<IndexResult, WalkerError> {
        let scan_opts = ScanOptions::new(path);
        let files = walk_path(&scan_opts)?;

        // Pre-warm parser pool (one parser per thread)
        let pool = ParserPool::new(opts.max_concurrency);
        let pool = Arc::new(pool);

        let (tx, rx) = mpsc::channel();

        // Use Rayon thread pool to process files in parallel
        files
            .par_iter()
            .enumerate()
            .with_max_len(1)
            .for_each_with(tx.clone(), |s, (idx, file)| {
                // Each worker can acquire a reference to parser pool (placeholder)
                let _parser = Arc::clone(&pool);

                // Simulate chunk generation (placeholder)
                let chunk = Chunk {
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
                };

                // Send chunk to collector
                let _ = s.send((chunk, file.clone()));
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
            .index_path_parallel(dir.path().to_str().unwrap(), opts)
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.chunks.len(), 1);
        let file = &result.files[0];
        assert_eq!(file.path, "lib.rs");
    }
}
