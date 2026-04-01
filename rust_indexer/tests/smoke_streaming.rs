use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use tempfile::tempdir;

fn spawn_indexer() -> Child {
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
fn smoke_index_path_streams_file_listed_events() {
    let mut child = spawn_indexer();
    let stdout = child.stdout.take().expect("child stdout");
    let mut reader = BufReader::new(stdout);
    let mut stdin = child.stdin.take().expect("child stdin");

    // consume capabilities
    let cap = read_next_event(&mut reader);
    assert_eq!(cap["event"], "capabilities");

    // create small tempdir with two files
    let td = tempdir().unwrap();
    std::fs::write(td.path().join("a.rs"), b"fn a() {}\n").unwrap();
    std::fs::write(td.path().join("b.txt"), b"hello\n").unwrap();

    let cmd = json!({
        "protocol_version": "1.0.0",
        "type": "command",
        "command": "index_path",
        "seq": 20,
        "job_id": "job-smoke-stream-1",
        "payload": {"path": td.path().to_str().unwrap(), "options": {"max_concurrency": 1}}
    });
    writeln!(stdin, "{}", cmd.to_string()).expect("failed to write command");

    let mut saw_file_listed = 0;
    let mut got_completed = false;
    for _ in 0..200 {
        let ev = read_next_event(&mut reader);
        let name = ev["event"].as_str().unwrap_or("");
        if name == "file_listed" && ev["job_id"] == "job-smoke-stream-1" {
            saw_file_listed += 1;
        }
        if name == "job_completed" && ev["job_id"] == "job-smoke-stream-1" {
            got_completed = true;
            break;
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(saw_file_listed >= 2, "expected at least two file_listed events");
    assert!(got_completed, "did not receive job_completed");
}
