use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

fn spawn_indexer() -> Child {
    // Try to locate built binary via CARGO_BIN_EXE; if not set (running via `cargo test`),
    // use rust_indexer/target/debug/rust_indexer relative to workspace root.
    if let Ok(exe) = std::env::var("CARGO_BIN_EXE_rust_indexer") {
        Command::new(exe)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn indexer")
    } else {
        let possible = std::path::PathBuf::from(std::env::current_dir().unwrap())
            .join("rust_indexer")
            .join("target")
            .join("debug")
            .join("rust_indexer");
        Command::new(possible)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn indexer")
    }
}

fn read_next_event(reader: &mut BufReader<std::process::ChildStdout>) -> Value {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("failed to read line from child stdout");
    serde_json::from_str(&line.trim()).expect("failed to parse json event")
}

#[test]
fn smoke_index_path_emits_job_events() {
    let mut child = spawn_indexer();
    let stdout = child.stdout.take().expect("child stdout");
    let mut reader = BufReader::new(stdout);
    let mut stdin = child.stdin.take().expect("child stdin");

    // capabilities should be the first emitted event
    let cap = read_next_event(&mut reader);
    assert_eq!(cap["event"], "capabilities");

    // send index_path command
    let cmd = json!({
        "protocol_version": "1.0.0",
        "type": "command",
        "command": "index_path",
        "seq": 10,
        "job_id": "job-smoke-1",
        "payload": {"path": ".", "options": {"max_concurrency": 1}}
    });
    writeln!(stdin, "{}", cmd.to_string()).expect("failed to write command");
    // keep stdin open to avoid the child exiting prematurely

    // expect job_started then job_completed
    let mut got_started = false;
    let mut got_completed = false;
    for _ in 0..10 {
        let ev = read_next_event(&mut reader);
        let name = ev["event"].as_str().unwrap_or("");
        if name == "job_started" && ev["job_id"] == "job-smoke-1" {
            got_started = true;
        }
        if name == "job_completed" && ev["job_id"] == "job-smoke-1" {
            got_completed = true;
            break;
        }
    }

    // cleanup
    let _ = child.kill();
    let _ = child.wait();

    assert!(got_started, "did not receive job_started");
    assert!(got_completed, "did not receive job_completed");
}

#[test]
fn smoke_unknown_command_emits_error() {
    let mut child = spawn_indexer();
    let stdout = child.stdout.take().expect("child stdout");
    let mut reader = BufReader::new(stdout);
    let mut stdin = child.stdin.take().expect("child stdin");

    // consume capabilities
    let cap = read_next_event(&mut reader);
    assert_eq!(cap["event"], "capabilities");

    // send unknown command
    let cmd = json!({
        "protocol_version": "1.0.0",
        "type": "command",
        "command": "this_command_does_not_exist",
        "seq": 11,
        "job_id": "job-smoke-2"
    });
    writeln!(stdin, "{}", cmd.to_string()).expect("failed to write command");

    // read next event which should be an error
    let ev = read_next_event(&mut reader);
    assert_eq!(ev["event"], "error");
    assert!(ev["payload"]["message"]
        .as_str()
        .unwrap_or("")
        .contains("unknown command"));

    let _ = child.kill();
    let _ = child.wait();
}
