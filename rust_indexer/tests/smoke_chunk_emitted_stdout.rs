use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;
use std::io::Write;

#[test]
fn binary_emits_chunk_emitted_in_stdout_for_index_path() {
    // Build a small temp repo with one file
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("lib.rs");
    std::fs::write(&file_path, b"fn main() { println!(\"hi\"); }\n").unwrap();

    // Run the binary with a JSONL command on stdin: index_path
    let mut cmd = Command::cargo_bin("rust_indexer").unwrap();
    let command = serde_json::json!({
        "protocol_version": "1.0.0",
        "type": "command",
        "command": "index_path",
        "job_id": "job-smoke-1",
        "payload": {"path": dir.path().to_str().unwrap(), "options": {"max_concurrency": 1}}
    });

    let mut child = cmd.write_stdin(command.to_string() + "\n").assert();

    // Expect stdout to eventually contain chunk_emitted event
    child.success().stdout(predicate::str::contains("\"event\":\"chunk_emitted\""));
}
