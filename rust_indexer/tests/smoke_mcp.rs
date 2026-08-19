use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn smoke_mcp_list_languages() {
    let mut child = Command::new("cargo")
        .args(["run", "--", "--mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    let stdin = child.stdin.as_mut().expect("stdin");
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"list_languages\",\"params\":{}}\n")
        .expect("write");
    let _ = stdin;

    let output = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let response: serde_json::Value = serde_json::from_str(stdout.trim()).expect("parse JSON");
    assert_eq!(response["jsonrpc"], "2.0", "jsonrpc version");
    assert_eq!(response["id"], 1, "id");
    assert!(response["result"].is_object(), "result exists");
    assert!(
        response["result"]["languages"].is_array(),
        "languages array"
    );
}

#[test]
fn smoke_mcp_index_path() {
    let mut child = Command::new("cargo")
        .args(["run", "--", "--mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    let stdin = child.stdin.as_mut().expect("stdin");
    stdin.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"index_path\",\"params\":{\"path\":\"./src\"}}\n").expect("write");
    let _ = stdin;

    let output = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let response = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|v| v.get("id").map(|id| id == 2).unwrap_or(false))
        .expect("find response with id 2");

    assert_eq!(response["jsonrpc"], "2.0", "jsonrpc version");
    assert_eq!(response["id"], 2, "id");
    assert!(response["result"]["job_id"].is_string(), "job_id returned");
}

#[test]
fn smoke_mcp_unknown_method() {
    let mut child = Command::new("cargo")
        .args(["run", "--", "--mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");

    let stdin = child.stdin.as_mut().expect("stdin");
    stdin
        .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"unknown_method\",\"params\":{}}\n")
        .expect("write");
    let _ = stdin;

    let output = child.wait_with_output().expect("wait");
    let stdout = String::from_utf8_lossy(&output.stdout);

    let response: serde_json::Value = serde_json::from_str(stdout.trim()).expect("parse JSON");
    assert_eq!(response["jsonrpc"], "2.0", "jsonrpc version");
    assert_eq!(response["id"], 3, "id");
    assert!(response["error"].is_object(), "error returned");
    assert_eq!(
        response["error"]["code"], -32601,
        "method not found error code"
    );
}
