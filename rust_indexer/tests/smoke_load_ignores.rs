//! CLI-level contract for `index_path` + `options.load_ignores`.
//!
//! `load_ignores: true` must (a) restrict the file set to the git-visible set
//! (tracked + untracked-not-ignored) and (b) FAIL with an explicit
//! `GIT_UNAVAILABLE` error when git/the repository is unavailable — never
//! degrade silently to a full scan.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdout, Command, Stdio};
use tempfile::tempdir;

fn spawn_indexer() -> Child {
    let exe = std::env::var("CARGO_BIN_EXE_rust_indexer")
        .expect("CARGO_BIN_EXE_rust_indexer must be set under `cargo test`");
    Command::new(exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn indexer")
}

fn read_event(reader: &mut BufReader<ChildStdout>) -> Value {
    let mut line = String::new();
    reader.read_line(&mut line).expect("read line");
    serde_json::from_str(line.trim()).expect("parse json event")
}

fn init_repo(path: &std::path::Path) {
    let p = path.to_str().unwrap();
    for args in [
        vec!["init"],
        vec!["config", "user.email", "t@example.com"],
        vec!["config", "user.name", "T"],
    ] {
        Command::new("git")
            .arg("-C")
            .arg(p)
            .args(&args)
            .output()
            .expect("git setup");
    }
}

/// Drives an `index_path` job to completion, returning every event emitted.
fn run_index_path(child: &mut Child, payload: Value) -> Vec<Value> {
    let stdout = child.stdout.take().expect("stdout");
    let mut reader = BufReader::new(stdout);
    let mut stdin = child.stdin.take().expect("stdin");

    // capabilities
    let _cap = read_event(&mut reader);

    let cmd = json!({
        "protocol_version": "1.0.0",
        "type": "command",
        "command": "index_path",
        "seq": 1,
        "job_id": "job-li-1",
        "payload": payload,
    });
    writeln!(stdin, "{}", cmd).expect("write command");

    let mut events = Vec::new();
    for _ in 0..500 {
        let ev = read_event(&mut reader);
        let name = ev["event"].as_str().unwrap_or("").to_string();
        events.push(ev);
        if name == "job_completed" {
            break;
        }
    }
    events
}

#[test]
fn index_path_load_ignores_true_skips_ignored_files() {
    let td = tempdir().unwrap();
    init_repo(td.path());
    std::fs::write(td.path().join(".gitignore"), "ignored.rs\n").unwrap();
    std::fs::write(td.path().join("kept.rs"), b"pub fn kept() {}\n").unwrap();
    std::fs::write(td.path().join("ignored.rs"), b"pub fn hidden() {}\n").unwrap();

    let mut child = spawn_indexer();
    let events = run_index_path(
        &mut child,
        json!({"path": td.path().to_str().unwrap(), "options": {"load_ignores": true}}),
    );
    let _ = child.kill();
    let _ = child.wait();

    let files: Vec<String> = events
        .iter()
        .filter(|e| e["event"] == "file_listed")
        .filter_map(|e| e["payload"]["file"]["path"].as_str().map(String::from))
        .collect();

    assert!(
        files.iter().any(|f| f == "kept.rs"),
        "expected kept.rs in {files:?}"
    );
    assert!(
        !files.iter().any(|f| f == "ignored.rs"),
        "ignored.rs leaked: {files:?}"
    );
}

#[test]
fn index_path_load_ignores_outside_repo_fails_explicitly() {
    let td = tempdir().unwrap();
    std::fs::write(td.path().join("a.rs"), b"pub fn a() {}\n").unwrap();

    let mut child = spawn_indexer();
    let events = run_index_path(
        &mut child,
        json!({"path": td.path().to_str().unwrap(), "options": {"load_ignores": true}}),
    );
    let _ = child.kill();
    let _ = child.wait();

    let err = events
        .iter()
        .find(|e| e["event"] == "error")
        .expect("expected an error event");
    assert_eq!(err["payload"]["code"], "GIT_UNAVAILABLE", "got: {err}");
    assert_eq!(err["payload"]["recoverable"], false);

    let completed = events
        .iter()
        .find(|e| e["event"] == "job_completed")
        .expect("expected job_completed");
    assert_eq!(completed["payload"]["processed"], 0);
}

#[test]
fn index_path_without_load_ignores_includes_everything() {
    let td = tempdir().unwrap();
    // No git repo: default (load_ignores absent) must still walk the tree.
    std::fs::write(td.path().join("a.rs"), b"pub fn a() {}\n").unwrap();
    std::fs::write(td.path().join("b.rs"), b"pub fn b() {}\n").unwrap();

    let mut child = spawn_indexer();
    let events = run_index_path(
        &mut child,
        json!({"path": td.path().to_str().unwrap(), "options": {}}),
    );
    let _ = child.kill();
    let _ = child.wait();

    assert!(
        events.iter().all(|e| e["event"] != "error"),
        "unexpected error without load_ignores: {events:?}"
    );
    let listed = events
        .iter()
        .filter(|e| e["event"] == "file_listed")
        .count();
    assert_eq!(listed, 2, "expected both files listed");
}
