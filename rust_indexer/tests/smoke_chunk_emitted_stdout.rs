use serde_json::Value;
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
fn smoke_index_path_emits_chunk_payloads_with_real_structure() {
    let mut child = spawn_indexer();
    let stdout = child.stdout.take().expect("child stdout");
    let mut reader = BufReader::new(stdout);
    let mut stdin = child.stdin.take().expect("child stdin");

    let capabilities = read_next_event(&mut reader);
    assert_eq!(capabilities["event"], "capabilities");

    let td = tempdir().unwrap();
    std::fs::write(
        td.path().join("lib.rs"),
        b"use std::fmt;\n\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
    )
    .unwrap();

    let cmd = serde_json::json!({
        "protocol_version": "1.0.0",
        "type": "command",
        "command": "index_path",
        "seq": 10,
        "job_id": "job-smoke-chunk-1",
        "payload": {"path": td.path().to_str().unwrap(), "options": {"max_concurrency": 1}}
    });
    writeln!(stdin, "{}", cmd.to_string()).expect("failed to write command");

    let mut saw_chunk = false;
    let mut got_completed = false;

    for _ in 0..2000 {
        let ev = read_next_event(&mut reader);
        match ev["event"].as_str().unwrap_or("") {
            "chunk_emitted" if ev["job_id"] == "job-smoke-chunk-1" => {
                let payload = &ev["payload"];
                assert_eq!(payload["file"], "lib.rs");
                assert_eq!(payload["language"], "rust");
                assert_eq!(payload["chunk_kind"], "Symbol");
                assert!(payload["chunk_id"].as_str().unwrap_or("").starts_with("chk-"));
                assert!(payload["symbol_id"].as_str().unwrap_or("").contains("add"));
                assert!(payload["text"].as_str().unwrap_or("").contains("use std::fmt;"));
                assert!(payload["text"].as_str().unwrap_or("").contains("pub fn add"));
                assert!(payload["start_line"].as_u64().unwrap_or(0) >= 1);
                assert!(payload["end_line"].as_u64().unwrap_or(0) >= payload["start_line"].as_u64().unwrap_or(0));
                assert!(payload["chunk_md5"].as_str().unwrap_or("").len() >= 8);
                assert!(payload["size"].as_u64().unwrap_or(0) > 0);
                saw_chunk = true;
            }
            "job_completed" if ev["job_id"] == "job-smoke-chunk-1" => {
                got_completed = true;
                break;
            }
            _ => {}
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(saw_chunk, "did not receive a chunk_emitted event");
    assert!(got_completed, "did not receive job_completed");
}
