pub trait ChunkStrategy {
    fn chunk_file(&self, file_path: &str, source: &str, symbols: Option<&Vec<crate::domain::types::Symbol>>) -> Vec<crate::domain::types::Chunk>;
}

pub mod overlap;
pub mod semantic;
pub mod size_limited;
pub mod symbol_boundary;
pub mod token_count;
pub mod token_limited;
pub mod with_context;

pub use overlap::OverlapChunker;
pub use semantic::SemanticChunker;
pub use size_limited::SizeLimitedChunker;
pub use symbol_boundary::SymbolBoundaryChunker;
pub use token_count::apply_token_count;
pub use token_limited::LineLimitedChunker;
pub use with_context::ContextInjectionChunker;
