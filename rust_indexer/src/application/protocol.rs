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
