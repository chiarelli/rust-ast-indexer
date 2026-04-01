pub mod dispatcher;

use std::io::{self, BufRead};
use std::thread;
use serde_json::json;

use crate::application::protocol::{Command, Event};
use crate::application::indexer::{Indexer, IndexOptions};
use crate::infra::jsonl;

pub fn run_cli() {
    // emit capabilities at startup
    let ev = Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "capabilities".into(),
        job_id: None,
        payload: Some(json!({"version":"0.1.0","languages":["rust","go","python","typescript","javascript","java"],"features":["jsonl","incremental_index","git_diff","pause_resume","mcp_compatible"]})),
    };
    jsonl::write_event(&ev);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(l) => {
                let l = l.trim();
                if l.is_empty() { continue; }
                // delegate to dispatcher module
                let handled = crate::cli::dispatcher::dispatch_line(l);
                if handled { continue; }

                // if dispatcher didn't handle, try to parse and forward to internal handler
                if let Some(cmd) = jsonl::read_command(l) {
                    handle_command(cmd);
                } else {
                    // malformed JSON already handled by dispatcher, but fallback emit
                    let ev = Event { protocol_version: "1.0.0".into(), r#type: "event".into(), event: "error".into(), job_id: None, payload: Some(json!({"code":"INVALID_COMMAND","message":"failed to parse command","recoverable":false})) };
                    jsonl::write_event(&ev);
                }
            }
            Err(_) => break,
        }
    }
}

#[allow(dead_code)]
fn handle_command(cmd: Command) {
    match cmd.command.as_str() {
        "list_languages" => {
            let ev = Event {
                protocol_version: "1.0.0".into(),
                r#type: "event".into(),
                event: "capabilities".into(),
                job_id: None,
                payload: Some(json!({"version":"0.1.0","languages":["rust","go","python","typescript","javascript","java"],"features":["jsonl","incremental_index","git_diff","pause_resume","mcp_compatible"]})),
            };
            jsonl::write_event(&ev);
        }
        "index_path" => {
            let job_id = cmd.job_id.clone().unwrap_or_else(|| "job-unknown".to_string());
            let payload = cmd.payload.unwrap_or_default();
            let path = payload.get("path").and_then(|v| v.as_str()).unwrap_or(".").to_string();
            let opts = IndexOptions { max_concurrency: payload.get("options").and_then(|o| o.get("max_concurrency")).and_then(|v| v.as_u64()).map(|v| v as usize).unwrap_or_else(num_cpus::get) };

            // spawn job thread
            thread::spawn(move || {
                let ev_start = Event {
                    protocol_version: "1.0.0".into(),
                    r#type: "event".into(),
                    event: "job_started".into(),
                    job_id: Some(job_id.clone()),
                    payload: Some(json!({"total_files":0})),
                };
                jsonl::write_event(&ev_start);

                let indexer = Indexer::new();
                let chunks = indexer.index_path(&path, opts);

                for chunk in &chunks {
                    let ev = Event {
                        protocol_version: "1.0.0".into(),
                        r#type: "event".into(),
                        event: "chunk_emitted".into(),
                        job_id: Some(job_id.clone()),
                        payload: Some(json!({
                            "chunk_id": chunk.id,
                            "chunk_kind": "Symbol",
                            "file": chunk.file_path,
                            "language": chunk.language,
                            "symbol_id": chunk.symbol_id,
                            "start_line": chunk.start_line,
                            "end_line": chunk.end_line,
                            "text": chunk.text,
                            "chunk_md5": chunk.md5,
                            "size": chunk.size
                        })),
                    };
                    jsonl::write_event(&ev);
                }

                let ev_done = Event {
                    protocol_version: "1.0.0".into(),
                    r#type: "event".into(),
                    event: "job_completed".into(),
                    job_id: Some(job_id.clone()),
                    payload: Some(json!({"processed": chunks.len(), "duration_ms": 0})),
                };
                jsonl::write_event(&ev_done);
            });
        }
        "dry_run" | "list_files" => {
            // simple acknowledgement
            let ev = Event {
                protocol_version: "1.0.0".into(),
                r#type: "event".into(),
                event: "job_progress".into(),
                job_id: cmd.job_id.clone(),
                payload: Some(json!({"processed_files":0,"total_files":0})),
            };
            jsonl::write_event(&ev);
        }
        "incremental_index" => {
            // For now, treat like index_path; caller may pass use_git/files in payload
            handle_command(cmd.with_command("index_path"));
        }
        "resume" => {
            // emit a resumed status (pause/resume handling is managed in job loop)
            let ev = Event { protocol_version: "1.0.0".into(), r#type: "event".into(), event: "job_progress".into(), job_id: cmd.job_id.clone(), payload: Some(json!({"is_paused": false})) };
            jsonl::write_event(&ev);
        }
        _ => {
            let ev = Event {
                protocol_version: "1.0.0".into(),
                r#type: "event".into(),
                event: "error".into(),
                job_id: cmd.job_id.clone(),
                payload: Some(json!({"code":"INVALID_COMMAND","message":format!("unknown command: {}", cmd.command),"recoverable":false})),
            };
            jsonl::write_event(&ev);
        }
    }
}

// helper to modify command via ownership; small convenience
#[allow(dead_code)]
trait CmdExt { fn with_command(self, c: &str) -> Command; }
impl CmdExt for Command {
    fn with_command(mut self, c: &str) -> Command { self.command = c.to_string(); self }
}
