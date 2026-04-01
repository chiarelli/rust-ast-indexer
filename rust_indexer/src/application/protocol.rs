use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Command {
    pub protocol_version: String,
    pub r#type: String,
    pub command: String,
    pub seq: Option<u64>,
    pub job_id: Option<String>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Event {
    pub protocol_version: String,
    pub r#type: String,
    pub event: String,
    pub job_id: Option<String>,
    pub payload: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Ack {
    pub protocol_version: String,
    pub r#type: String,
    pub seq: Option<u64>,
    pub job_id: Option<String>,
    pub payload: Option<serde_json::Value>,
}

// Chunk event schema and variants
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum ChunkKind {
    FullFile,
    Symbol,
    Contextual,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChunkEventPayload {
    pub chunk_id: String,
    pub chunk_kind: ChunkKind,
    // relative file path
    pub file: String,
    pub language: Option<String>,
    pub symbol_id: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    #[serde(rename = "chunk_md5")]
    pub chunk_md5: String,
    pub size: usize,
}

impl From<crate::domain::types::Chunk> for ChunkEventPayload {
    fn from(c: crate::domain::types::Chunk) -> Self {
        // map chunk_kind string to enum, default to Contextual if unknown
        let kind = c.chunk_kind.and_then(|s| match s.as_str() {
            "FullFile" => Some(ChunkKind::FullFile),
            "Symbol" => Some(ChunkKind::Symbol),
            "Contextual" => Some(ChunkKind::Contextual),
            other => {
                // try case-insensitive match
                match other.to_lowercase().as_str() {
                    "fullfile" => Some(ChunkKind::FullFile),
                    "symbol" => Some(ChunkKind::Symbol),
                    "contextual" => Some(ChunkKind::Contextual),
                    _ => None,
                }
            }
        }).unwrap_or(ChunkKind::Contextual);

        ChunkEventPayload {
            chunk_id: c.id,
            chunk_kind: kind,
            file: c.file_path,
            language: c.language,
            symbol_id: c.symbol_id,
            start_line: c.start_line,
            end_line: c.end_line,
            text: c.text,
            chunk_md5: c.md5,
            size: c.size,
        }
    }
}
