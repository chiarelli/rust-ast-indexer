use rust_indexer::cli::dispatcher;
use rust_indexer::application::protocol::Command;

#[test]
fn integration_dispatch_cmd_list_languages() {
    let cmd = Command { protocol_version: "1.0.0".into(), r#type: "command".into(), command: "list_languages".into(), seq: Some(1), job_id: None, payload: None };
    assert!(dispatcher::dispatch_cmd(&cmd));
}

#[test]
fn integration_dispatch_cmd_unknown() {
    let cmd = Command { protocol_version: "1.0.0".into(), r#type: "command".into(), command: "unknown_cmd".into(), seq: Some(2), job_id: None, payload: None };
    assert!(!dispatcher::dispatch_cmd(&cmd));
}
