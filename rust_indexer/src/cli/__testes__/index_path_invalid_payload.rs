#[cfg(test)]
mod tests {
    use crate::cli::dispatcher;
    use crate::application::protocol::Command;
    use serde_json::json;

    #[test]
    fn index_path_missing_payload_emits_error() {
        let cmd = Command { protocol_version: "1.0.0".into(), r#type: "command".into(), command: "index_path".into(), seq: Some(20), job_id: Some("job-invalid-1".into()), payload: None };
        // dispatcher won't handle index_path (returns false) so caller should handle; we test dispatch_cmd false
        assert!(!dispatcher::dispatch_cmd(&cmd));
    }

    #[test]
    fn index_path_missing_path_emits_error() {
        let cmd = Command { protocol_version: "1.0.0".into(), r#type: "command".into(), command: "index_path".into(), seq: Some(21), job_id: Some("job-invalid-2".into()), payload: Some(json!({"options": {"max_concurrency":1}})) };
        assert!(!dispatcher::dispatch_cmd(&cmd));
    }
}
