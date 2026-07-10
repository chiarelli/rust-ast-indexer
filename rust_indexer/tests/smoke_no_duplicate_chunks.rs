use serde_json::Value;
use std::collections::HashSet;
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

fn run_index_path_test(job_id: &str, dir: &std::path::Path) {
    let mut child = spawn_indexer();
    let stdout = child.stdout.take().expect("child stdout");
    let mut reader = BufReader::new(stdout);
    let mut stdin = child.stdin.take().expect("child stdin");

    let capabilities = read_next_event(&mut reader);
    assert_eq!(capabilities["event"], "capabilities");

    let cmd = serde_json::json!({
        "protocol_version": "1.0.0",
        "type": "command",
        "command": "index_path",
        "seq": 10,
        "job_id": job_id,
        "payload": {
            "path": dir.to_str().unwrap(),
            "options": {
                "max_concurrency": 1,
                "extract_imports": false,
                "extract_calls": false,
                "chunking": {
                    "strategy": "semantic",
                    "max_lines": 200,
                    "overlap_lines": 1,
                    "include_context": true
                }
            }
        }
    });
    writeln!(stdin, "{}", cmd).expect("failed to write command");

    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut duplicate_ids: Vec<String> = Vec::new();
    let mut got_completed = false;

    for _ in 0..2000 {
        let ev = read_next_event(&mut reader);
        match ev["event"].as_str().unwrap_or("") {
            "chunk_emitted" if ev["job_id"] == job_id => {
                let chunk_id = ev["payload"]["chunk_id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if !seen_ids.insert(chunk_id.clone()) {
                    duplicate_ids.push(chunk_id);
                }
            }
            "job_completed" if ev["job_id"] == job_id => {
                got_completed = true;
                break;
            }
            _ => {}
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(got_completed, "did not receive job_completed");
    assert!(
        !seen_ids.is_empty(),
        "expected at least one chunk_emitted event"
    );
    assert!(
        duplicate_ids.is_empty(),
        "found duplicate chunk_emitted events for chunk_ids: {:?}",
        duplicate_ids
    );
}

#[test]
fn smoke_index_path_no_duplicate_chunks_rust_file() {
    let td = tempdir().unwrap();
    std::fs::write(
        td.path().join("lib.rs"),
        b"use std::fmt;\n\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\npub fn sub(a: i32, b: i32) -> i32 {\n    a - b\n}\n",
    )
    .unwrap();
    run_index_path_test("job-no-dup-1", td.path());
}

#[test]
fn smoke_index_path_no_duplicate_chunks_multi_file() {
    let td = tempdir().unwrap();
    std::fs::write(td.path().join("a.rs"), b"fn a() {}\n").unwrap();
    std::fs::write(td.path().join("b.rs"), b"fn b() {}\n").unwrap();
    std::fs::write(td.path().join("c.rs"), b"fn c() {}\n").unwrap();
    run_index_path_test("job-no-dup-2", td.path());
}

#[test]
fn smoke_index_path_no_duplicate_chunks_unicode_python() {
    let td = tempdir().unwrap();
    std::fs::write(
        td.path().join("test_mod.py"),
        b"# -*- coding: utf-8 -*-\n\"\"\"Test module with unicode.\"\"\"\n\ndef funcao_com_acentos():\n    \"\"\"Fun\xc3\xa7\xc3\xa3o com acentos.\"\"\"\n    print(\"Ol\xc3\xa1, mundo!\")\n\nclass ClasseExemplo:\n    def metodo(self):\n        pass\n",
    )
    .unwrap();
    run_index_path_test("job-no-dup-3", td.path());
}
