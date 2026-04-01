use crate::application::protocol::Event;
use crate::infra::jsonl;
use serde_json::json;

// Attempt to handle a line entirely in the dispatcher. Returns true if the
// dispatcher handled the command, false if the caller should process it.
pub fn dispatch_line(line: &str) -> bool {
    if let Some(cmd) = jsonl::read_command(line) {
        match cmd.command.as_str() {
            "list_languages" => {
                let ev = Event {
                    protocol_version: "1.0.0".into(),
                    r#type: "event".into(),
                    event: "capabilities".into(),
                    job_id: None,
                    payload: Some(json!({"version":"0.1.0","languages":["rust","go","python","typescript","javascript","java"],"features":["jsonl","incremental_index","git_diff","pause_resume","mcp_compatible"]})),
                };
                jsonl::write_event(&ev);
                true
            }
            _ => {
                // not handled by dispatcher
                false
            }
        }
    } else {
        let ev = Event {
            protocol_version: "1.0.0".into(),
            r#type: "event".into(),
            event: "error".into(),
            job_id: None,
            payload: Some(json!({"code":"INVALID_COMMAND","message":"failed to parse command","recoverable":false})),
        };
        jsonl::write_event(&ev);
        true
    }
}
