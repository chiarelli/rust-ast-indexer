use serde::{Deserialize, Serialize};

/// Supported chunking strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChunkingStrategy {
    /// One chunk per symbol (function, class, struct, etc.)
    SymbolBoundary,
    /// Group related symbols semantically (impls with structs, methods with classes)
    Semantic,
    /// Break chunks at line boundaries with size limits
    LineLimited,
}

/// Configuration options for chunk generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkingOptions {
    /// Chunking strategy to use
    pub strategy: ChunkingStrategy,
    /// Maximum lines per chunk (applies to size-limited strategies)
    pub max_lines: usize,
    /// Number of lines to overlap between consecutive chunks
    pub overlap_lines: usize,
    /// Whether to inject context (imports, parent scope) as prefix
    pub include_context: bool,
    /// Whether to count tokens in chunks (requires token_counting feature)
    pub token_counting: bool,
}

impl Default for ChunkingOptions {
    fn default() -> Self {
        Self {
            strategy: ChunkingStrategy::Semantic,
            max_lines: 200,
            overlap_lines: 1,
            include_context: true,
            token_counting: false,
        }
    }
}