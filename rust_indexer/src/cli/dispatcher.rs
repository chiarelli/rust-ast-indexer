use crate::application::protocol::{Command, Event};
use crate::infra::jsonl;
use serde_json::json;

// Dispatch a raw line: parse command, then try to handle via dispatch_cmd.
// Returns true if handled (either dispatched or error emitted), false otherwise.
pub fn dispatch_line(line: &str) -> bool {
    if let Some(cmd) = jsonl::read_command(line) {
        dispatch_cmd(&cmd)
    } else {
        let ev = Event {
            protocol_version: "1.0.0".into(),
            r#type: "event".into(),
            event: "error".into(),
            job_id: None,
            payload: Some(
                json!({"code":"INVALID_COMMAND","message":"failed to parse command","recoverable":false}),
            ),
        };
        jsonl::write_event(&ev);
        true
    }
}

// Attempt to handle a parsed Command in the dispatcher. Returns true if the
// dispatcher handled the command, false if the caller should process it.
pub fn dispatch_cmd(cmd: &Command) -> bool {
    match cmd.command.as_str() {
        "list_languages" => {
            let ev = Event {
                protocol_version: "1.0.0".into(),
                r#type: "event".into(),
                event: "capabilities".into(),
                job_id: None,
                payload: Some(
                    json!({"version":"0.1.0","languages":["rust","go","python","typescript","javascript","java"],"features":["jsonl","incremental_index","git_diff","pause_resume","mcp_compatible"]}),
                ),
            };
            jsonl::write_event(&ev);
            true
        }
        _ => false,
    }
}
