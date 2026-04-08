use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use serde_json::Value;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileRecord {
    pub path: String,
    pub size: u64,
    pub mtime: u64,
    pub hash: String,
    pub language: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Symbol {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub scope: Option<String>,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NormalizedSymbol {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub qualified_name: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
    pub language: Option<String>,
    pub is_overloaded: bool,
    pub overload_index: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Chunk {
    pub id: String,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    // Primary textual content of the chunk. Keep `text` for backwards compatibility with existing code.
    pub content: String,
    pub text: String,
    // md5 of the chunk content
    pub md5: String,
    pub size: usize,
    pub language: Option<String>,
    // Primary associated symbol (if any)
    pub symbol_id: Option<String>,
    // All related symbols within this chunk
    pub symbol_ids: Vec<String>,
    pub chunk_kind: Option<String>,
    // Extensible metadata map for additional attributes
    pub metadata: Option<HashMap<String, Value>>,
}

impl Chunk {
    /// Validate basic invariants for a Chunk
    pub fn validate(&self) -> Result<(), String> {
        if self.start_line == 0 || self.end_line == 0 {
            return Err("start_line and end_line must be >= 1".into());
        }
        if self.start_line > self.end_line {
            return Err("start_line must be <= end_line".into());
        }
        Ok(())
    }
}

impl std::fmt::Display for Chunk {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Chunk {{ id: {}, file: {}, lines: {}-{} }}", self.id, self.file_path, self.start_line, self.end_line)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportEdge {
    pub id: String,
    pub from_file: String,
    pub to_module: String,
    pub imported_symbol: Option<String>,
    pub alias: Option<String>,
    pub import_kind: String,
    pub location: Location,
    pub resolved: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CallEdge {
    pub id: String,
    pub caller_symbol_id: Option<String>,
    pub callee_name: String,
    pub callee_symbol_id: Option<String>,
    pub call_kind: String,
    pub location: Location,
    pub resolved: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Location {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}
