use crate::application::protocol::{ChunkEventPayload, Command, Event};
use crate::domain::types::{CallEdge, ImportEdge};

pub fn write_event(e: &Event) {
    let s = serde_json::to_string(e).unwrap_or_else(|_| "{}".into());
    println!("{}", s);
}

pub fn build_chunk_event(job_id: Option<String>, payload: &ChunkEventPayload) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "chunk_emitted".into(),
        job_id,
        payload: Some(serde_json::to_value(payload).unwrap_or(serde_json::Value::Null)),
    }
}

pub fn write_chunk_event(job_id: Option<String>, payload: &ChunkEventPayload) {
    let ev = build_chunk_event(job_id, payload);
    write_event(&ev);
}

pub fn build_import_event(job_id: Option<String>, payload: &ImportEdge) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "import_edge".into(),
        job_id,
        payload: Some(serde_json::to_value(payload).unwrap_or(serde_json::Value::Null)),
    }
}

pub fn write_import_event(job_id: Option<String>, payload: &ImportEdge) {
    let ev = build_import_event(job_id, payload);
    write_event(&ev);
}

pub fn build_call_event(job_id: Option<String>, payload: &CallEdge) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "call_edge".into(),
        job_id,
        payload: Some(serde_json::to_value(payload).unwrap_or(serde_json::Value::Null)),
    }
}

pub fn write_call_event(job_id: Option<String>, payload: &CallEdge) {
    let ev = build_call_event(job_id, payload);
    write_event(&ev);
}

pub fn read_command(line: &str) -> Option<Command> {
    serde_json::from_str(line).ok()
}

// Backpressure events
pub fn build_pause_event(job_id: Option<String>) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "pause".into(),
        job_id,
        payload: None,
    }
}

pub fn write_pause_event(job_id: Option<String>) {
    let ev = build_pause_event(job_id);
    write_event(&ev);
}

pub fn build_resume_event(job_id: Option<String>) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "resume".into(),
        job_id,
        payload: None,
    }
}

pub fn write_resume_event(job_id: Option<String>) {
    let ev = build_resume_event(job_id);
    write_event(&ev);
}

// Backpressure events with payload
pub fn build_pause_event_with_payload(job_id: Option<String>, payload: serde_json::Value) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "pause".into(),
        job_id,
        payload: Some(payload),
    }
}

pub fn write_pause_event_with_payload(job_id: Option<String>, payload: serde_json::Value) {
    let ev = build_pause_event_with_payload(job_id, payload);
    write_event(&ev);
}

pub fn build_resume_event_with_payload(job_id: Option<String>, payload: serde_json::Value) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "resume".into(),
        job_id,
        payload: Some(payload),
    }
}

pub fn write_resume_event_with_payload(job_id: Option<String>, payload: serde_json::Value) {
    let ev = build_resume_event_with_payload(job_id, payload);
    write_event(&ev);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_pause_event_contains_metadata() {
        let ev = build_pause_event(Some("job-1".into()));
        assert_eq!(ev.event, "pause");
        assert_eq!(ev.job_id.as_deref(), Some("job-1"));
        assert!(ev.payload.is_none());
    }

    #[test]
    fn build_resume_event_contains_metadata() {
        let ev = build_resume_event(Some("job-1".into()));
        assert_eq!(ev.event, "resume");
        assert_eq!(ev.job_id.as_deref(), Some("job-1"));
        assert!(ev.payload.is_none());
    }

    #[test]
    fn build_chunk_event_contains_payload_and_metadata() {
        let payload = ChunkEventPayload {
            chunk_id: "chunk-1".into(),
            chunk_kind: crate::application::protocol::ChunkKind::FullFile,
            file: "src/lib.rs".into(),
            language: Some("rust".into()),
            symbol_id: Some("sym-1".into()),
            start_line: 1,
            end_line: 10,
            text: "fn main() {}".into(),
            chunk_md5: "abc123".into(),
            size: 12,
        };

        let ev = build_chunk_event(Some("job-1".into()), &payload);
        assert_eq!(ev.event, "chunk_emitted");
        assert_eq!(ev.job_id.as_deref(), Some("job-1"));
        let val = ev.payload.unwrap();
        let expected = serde_json::to_value(&payload).unwrap();
        assert_eq!(val, expected);
    }
}
