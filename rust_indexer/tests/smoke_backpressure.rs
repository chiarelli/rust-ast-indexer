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
        let possible = std::env::current_dir()
            .unwrap()
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
    serde_json::from_str(line.trim()).expect("failed to parse json event")
}

#[test]
fn smoke_backpressure_emits_pause_and_resume_events() {
    let mut child = spawn_indexer();
    let stdout = child.stdout.take().expect("child stdout");
    let mut reader = BufReader::new(stdout);
    let mut stdin = child.stdin.take().expect("child stdin");

    // consume capabilities
    let cap = read_next_event(&mut reader);
    assert_eq!(cap["event"], "capabilities");

    // create tempdir with many small files to force queue filling
    let td = tempdir().unwrap();
    for i in 0..50 {
        let path = td.path().join(format!("file_{}.rs", i));
        std::fs::write(path, format!("fn f_{}() {{}}\n", i)).unwrap();
    }

    let cmd = json!({
        "protocol_version": "1.0.0",
        "type": "command",
        "command": "index_path",
        "seq": 1,
        "job_id": "job-backpressure-test-1",
        "payload": {
            "path": td.path().to_str().unwrap(),
            "options": {
                "max_concurrency": 1,
                "backpressure": {
                    "max_queue_size": 10,
                    "threshold_percent": 80,
                    "ack_required": false,
                    "pause_timeout_secs": 30
                }
            }
        }
    });
    writeln!(stdin, "{}", cmd).expect("failed to write command");

    let mut saw_pause = false;
    let mut saw_resume = false;
    let mut got_completed = false;

    for _ in 0..500 {
        let ev = read_next_event(&mut reader);
        let name = ev["event"].as_str().unwrap_or("");

        if name == "pause" && ev["job_id"] == "job-backpressure-test-1" {
            saw_pause = true;
            assert_eq!(ev["payload"]["reason"], "output_queue_full");
            assert!(ev["payload"]["current_size"].as_u64().unwrap() >= 10);
        }

        if name == "resume" && ev["job_id"] == "job-backpressure-test-1" {
            saw_resume = true;
            assert_eq!(ev["payload"]["reason"], "queue_under_threshold");
            assert!(ev["payload"]["current_size"].as_u64().unwrap() <= 8);
        }

        if name == "job_completed" && ev["job_id"] == "job-backpressure-test-1" {
            got_completed = true;
            break;
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(saw_pause, "pause event was not emitted under load");
    assert!(
        saw_resume,
        "resume event was not emitted after queue cleared"
    );
    assert!(got_completed, "job did not complete successfully");
}
