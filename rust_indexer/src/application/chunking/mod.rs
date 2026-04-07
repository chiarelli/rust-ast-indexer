pub trait ChunkStrategy {
    fn chunk_file(&self, file_path: &str, source: &str, symbols: Option<&Vec<crate::domain::types::Symbol>>) -> Vec<crate::domain::types::Chunk>;
}

pub mod symbol_boundary;

pub use symbol_boundary::SymbolBoundaryChunker;
