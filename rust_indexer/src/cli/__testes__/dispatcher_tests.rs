#[cfg(test)]
mod tests {
    use crate::cli::dispatcher;
    use crate::application::protocol::Command;
    use serde_json::json;

    #[test]
    fn dispatch_list_languages_returns_true() {
        let line = r#"{"protocol_version":"1.0.0","type":"command","command":"list_languages","seq":1}"#;
        let handled = dispatcher::dispatch_line(line);
        assert!(handled, "list_languages should be handled by dispatcher");
    }

    #[test]
    fn dispatch_unknown_returns_false() {
        let line = r#"{"protocol_version":"1.0.0","type":"command","command":"unknown_cmd","seq":2}"#;
        let handled = dispatcher::dispatch_line(line);
        assert!(!handled, "unknown commands should not be handled by dispatcher");
    }

    #[test]
    fn dispatch_invalid_json_returns_true() {
        let line = "not a json";
        let handled = dispatcher::dispatch_line(line);
        assert!(handled, "invalid json should be handled (emit error)");
    }

    #[test]
    fn dispatch_cmd_struct_works() {
        let cmd = Command { protocol_version: "1.0.0".into(), r#type: "command".into(), command: "list_languages".into(), seq: Some(1), job_id: None, timestamp: None, payload: None };
        let handled = dispatcher::dispatch_cmd(&cmd);
        assert!(handled);
    }
}
