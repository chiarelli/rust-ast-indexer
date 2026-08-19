use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::tempdir;

#[test]
fn binary_indexes_files_from_git_tracked_and_diff() {
    // Create a temporary git repo
    let dir = tempdir().unwrap();
    let root = dir.path();

    // init git repo and config
    std::process::Command::new("git").arg("-C").arg(root).arg("init").output().unwrap();
    std::process::Command::new("git").arg("-C").arg(root).args(["config","user.email","you@example.com"]).output().unwrap();
    std::process::Command::new("git").arg("-C").arg(root).args(["config","user.name","Tester"]).output().unwrap();

    // add initial file and commit
    std::fs::write(root.join("main.rs"), b"fn main() {}").unwrap();
    std::process::Command::new("git").arg("-C").arg(root).arg("add").arg(".").output().unwrap();
    std::process::Command::new("git").arg("-C").arg(root).arg("commit").arg("-m").arg("init").output().unwrap();
    std::process::Command::new("git").arg("-C").arg(root).arg("tag").arg("v1").output().unwrap();

    // modify file and commit second
    std::fs::write(root.join("main.rs"), b"fn main() { println!(\"hello\"); }").unwrap();
    std::process::Command::new("git").arg("-C").arg(root).arg("add").arg(".").output().unwrap();
    std::process::Command::new("git").arg("-C").arg(root).arg("commit").arg("-m").arg("second").output().unwrap();

    // Run binary with incremental_index payload using git_range from v1 to HEAD
    let mut cmd = Command::cargo_bin("rust_indexer").unwrap();
    let command = serde_json::json!({
        "protocol_version": "1.0.0",
        "type": "command",
        "command": "incremental_index",
        "job_id": "job-smoke-git-1",
        "payload": {"path": root.to_str().unwrap(), "use_git": true, "git_range": {"from": "v1", "to": "HEAD"}, "options": {"max_concurrency": 1}}
    });

    let child = cmd.write_stdin(command.to_string() + "\n").assert();
    child.success().stdout(predicate::str::contains("\"event\":\"chunk_emitted\""));
}
