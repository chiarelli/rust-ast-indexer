pub mod normalize;
pub mod parser;
pub mod types;

// Domain-level re-exports
pub use normalize::normalize_symbols;
pub use types::{CallEdge, Chunk, FileRecord, ImportEdge, Location, NormalizedSymbol, Symbol};
