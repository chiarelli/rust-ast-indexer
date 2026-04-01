use crate::domain::types::{Chunk, FileRecord};
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
}
