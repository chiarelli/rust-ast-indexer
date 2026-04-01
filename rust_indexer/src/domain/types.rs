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
