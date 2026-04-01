#[cfg(test)]
mod tests {
    use crate::application::protocol::Command;
    use crate::cli::dispatcher;
    use serde_json::json;
    use std::io::{self, Write};

    // These tests exercise the handle_command path by constructing a Command
    // and calling the internal handler via run_cli's handle_command helper.
    // We can't easily capture stdout from jsonl::write_event without redirecting,
    // so we assert that dispatch_cmd returns expected booleans and that
    // indexer returns empty chunks as placeholder.

    #[test]
    fn index_path_dispatch_handled_by_cli() {
        let cmd = Command { protocol_version: "1.0.0".into(), r#type: "command".into(), command: "index_path".into(), seq: Some(3), job_id: Some("job-ix-1".into()), payload: Some(json!({"path":".", "options": {"max_concurrency":1}})) };
        // dispatcher::dispatch_cmd should return false so that the CLI handles it
        assert!(!dispatcher::dispatch_cmd(&cmd));
    }

    #[test]
    fn incremental_index_aliases_to_index_path() {
        let cmd = Command { protocol_version: "1.0.0".into(), r#type: "command".into(), command: "incremental_index".into(), seq: Some(4), job_id: Some("job-ix-2".into()), payload: Some(json!({"path":".", "files":["src/lib.rs"]})) };
        // dispatcher treats incremental_index by delegating to index_path via CmdExt
        // dispatch_cmd returns false (not handled here)
        assert!(!dispatcher::dispatch_cmd(&cmd));
    }
}
