use crate::application::protocol::{Command, Event, ChunkEventPayload};

pub fn write_event(e: &Event) {
    let s = serde_json::to_string(e).unwrap_or_else(|_| "{}".into());
    println!("{}", s);
}

pub fn write_chunk_event(job_id: Option<String>, payload: &ChunkEventPayload) {
    let ev = Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "chunk_emitted".into(),
        job_id,
        payload: Some(serde_json::to_value(payload).unwrap_or(serde_json::Value::Null)),
    };
    write_event(&ev);
}

pub fn read_command(line: &str) -> Option<Command> {
    serde_json::from_str(line).ok()
}
