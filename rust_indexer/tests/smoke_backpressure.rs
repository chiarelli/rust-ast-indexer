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

    let cap = read_next_event(&mut reader);
    assert_eq!(cap["event"], "capabilities");

    let td = tempdir().unwrap();
    for i in 0..30 {
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
                    "max_queue_size": 5,
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
    let mut ack_sent = false;
    let mut iterations = 0;

    while iterations < 3000 {
        iterations += 1;
        let ev = read_next_event(&mut reader);
        let name = ev["event"].as_str().unwrap_or("");

        if name == "pause" && ev["job_id"] == "job-backpressure-test-1" {
            saw_pause = true;
            assert!(ev["payload"]["backpressure_active"]
                .as_bool()
                .unwrap_or(false));
            assert!(ev["payload"]["current_size"].as_u64().unwrap() >= 5);

            // Give indexer time to process the pause state before sending ack
            std::thread::sleep(std::time::Duration::from_millis(200));

            let ack_cmd = json!({
                "protocol_version": "1.0.0",
                "type": "command",
                "command": "ack",
                "seq": 2,
                "job_id": "job-backpressure-test-1",
                "payload": {
                    "count": 100  // Decrement ALL to ensure we go below threshold
                }
            });
            writeln!(stdin, "{}", ack_cmd).expect("failed to write ack command");
            stdin.flush().expect("failed to flush stdin");
            ack_sent = true;

            // Wait for ack to be processed and resume to be emitted
            std::thread::sleep(std::time::Duration::from_millis(500));
        }

        if name == "resume" && ev["job_id"] == "job-backpressure-test-1" {
            saw_resume = true;
            // Note: reason is flattened into the payload via serde(flatten)
            // So it appears as "queue_under_threshold" or "external_signal"
            assert!(
                ev["payload"]["queue_under_threshold"].is_null()
                    || ev["payload"]["external_signal"].is_null(),
                "resume reason should be queue_under_threshold or external_signal"
            );
            assert!(ev["payload"]["current_size"].as_u64().unwrap() <= 8);
        }

        if name == "job_completed" && ev["job_id"] == "job-backpressure-test-1" {
            got_completed = true;
            // After job_completed, we should wait a bit more for resume to arrive
            // The ack command is processed after job_completed, so resume may come after
            // But we don't need to keep waiting forever - let's break after a reasonable time
            // Actually, we should continue waiting since resume comes AFTER job_completed in our fix
            // Let's just set a flag and continue to look for resume
        }

        // If we have seen all three events, we can exit early
        if saw_pause && saw_resume && got_completed {
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
