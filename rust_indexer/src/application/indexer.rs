use crate::domain::types::Chunk;

pub struct IndexOptions {
    pub max_concurrency: usize,
}

pub struct Indexer {}

impl Default for Indexer {
    fn default() -> Self {
        Self::new()
    }
}

impl Indexer {
    pub fn new() -> Self { Indexer {} }
    pub fn index_path(&self, _path: &str, _opts: IndexOptions) -> Vec<Chunk> {
        // placeholder
        Vec::new()
    }
}
