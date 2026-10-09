pub mod chunk_tests;
pub mod normalize;
pub mod parser;
pub mod resolve;
pub mod types;

// Domain-level re-exports
pub use normalize::normalize_import;
pub use normalize::normalize_symbols;
pub use resolve::{apply_import_resolution, resolve_import_target};
pub use types::{CallEdge, Chunk, FileRecord, ImportEdge, Location, NormalizedSymbol, Symbol};
