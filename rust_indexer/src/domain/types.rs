use serde::{Deserialize, Serialize};

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
    pub text: String,
    pub md5: String,
    pub size: usize,
    pub language: Option<String>,
    pub symbol_id: Option<String>,
    pub chunk_kind: Option<String>,
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
