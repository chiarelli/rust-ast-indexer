pub trait ChunkStrategy {
    fn chunk_file(&self, file_path: &str, source: &str, symbols: Option<&Vec<crate::domain::types::Symbol>>) -> Vec<crate::domain::types::Chunk>;
}

impl<T: ChunkStrategy + ?Sized> ChunkStrategy for Box<T> {
    fn chunk_file(&self, file_path: &str, source: &str, symbols: Option<&Vec<crate::domain::types::Symbol>>) -> Vec<crate::domain::types::Chunk> {
        (**self).chunk_file(file_path, source, symbols)
    }
}

pub mod overlap;
pub mod semantic;
pub mod size_limited;
pub mod strategy;
pub mod symbol_boundary;
pub mod token_count;
pub mod token_limited;
pub mod with_context;

pub use overlap::OverlapChunker;
pub use semantic::SemanticChunker;
pub use size_limited::SizeLimitedChunker;
pub use strategy::{ChunkingOptions, ChunkingStrategy};
pub use symbol_boundary::SymbolBoundaryChunker;
pub use token_count::apply_token_count;
pub use token_limited::LineLimitedChunker;
pub use with_context::ContextInjectionChunker;
