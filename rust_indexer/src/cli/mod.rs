pub mod args;
pub mod dispatcher;

#[cfg(test)]
mod __testes__;

use serde_json::json;
use std::io::{self, BufRead};
use std::thread;

use crate::application::indexer::{IndexOptions, Indexer};
use crate::application::protocol::{Command, Event};
use crate::infra::jsonl;

use std::sync::Arc;

use crate::app::bootstrap::{init_context, ApplicationContext, Config};

pub fn run_cli_default() {
    let cfg = Config::load();
    let ctx = init_context(cfg);
    run_cli(ctx);
}

pub fn run_cli(ctx: Arc<ApplicationContext>) {
    // emit capabilities at startup
    let languages = ctx.registry.list_languages();
    let ev = Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "capabilities".into(),
        job_id: None,
        payload: Some(
            json!({"version":"0.1.0","languages":languages,"features":["jsonl","incremental_index","git_diff","pause_resume","mcp_compatible"]}),
        ),
    };
    jsonl::write_event(&ev);

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(l) => {
                let l = l.trim();
                if l.is_empty() {
                    continue;
                }
                // delegate to dispatcher module
                let handled = crate::cli::dispatcher::dispatch_line(l);
                if handled {
                    continue;
                }

                // if dispatcher didn't handle, try to parse and forward to internal handler
                if let Some(cmd) = jsonl::read_command(l) {
                    handle_command(ctx.clone(), cmd);
                } else {
                    // malformed JSON already handled by dispatcher, but fallback emit
                    let ev = Event {
                        protocol_version: "1.0.0".into(),
                        r#type: "event".into(),
                        event: "error".into(),
                        job_id: None,
                        payload: Some(
                            json!({"code":"INVALID_COMMAND","message":"failed to parse command","recoverable":false}),
                        ),
                    };
                    jsonl::write_event(&ev);
                }
            }
            Err(_) => break,
        }
    }

    // Give spawned background job threads a short grace period to emit events before exiting.
    // This keeps the binary usable as a short-lived child process in smoke tests.
    std::thread::sleep(std::time::Duration::from_millis(100));
}

#[allow(dead_code)]
pub fn handle_command(ctx: Arc<ApplicationContext>, cmd: Command) {
    match cmd.command.as_str() {
        "list_languages" => {
            let languages = ctx.registry.list_languages();
            let ev = Event {
                protocol_version: "1.0.0".into(),
                r#type: "event".into(),
                event: "capabilities".into(),
                job_id: None,
                payload: Some(
                    json!({"version":"0.1.0","languages":languages,"features":["jsonl","incremental_index","git_diff","pause_resume","mcp_compatible"]}),
                ),
            };
            jsonl::write_event(&ev);
        }
        "index_path" => {
            let job_id = cmd
                .job_id
                .clone()
                .unwrap_or_else(|| "job-unknown".to_string());
            // validate payload exists and contains a path string
            let payload = match cmd.payload {
                Some(p) => p,
                None => {
                    let ev = Event {
                        protocol_version: "1.0.0".into(),
                        r#type: "event".into(),
                        event: "error".into(),
                        job_id: cmd.job_id.clone(),
                        payload: Some(
                            json!({"code":"INVALID_PAYLOAD","message":"missing payload for index_path","recoverable":false}),
                        ),
                    };
                    jsonl::write_event(&ev);
                    return;
                }
            };
            let path = payload
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if path.is_empty() {
                let ev = Event {
                    protocol_version: "1.0.0".into(),
                    r#type: "event".into(),
                    event: "error".into(),
                    job_id: cmd.job_id.clone(),
                    payload: Some(
                        json!({"code":"INVALID_PAYLOAD","message":"missing path in payload","recoverable":false}),
                    ),
                };
                jsonl::write_event(&ev);
                return;
            }
            let chunking_opts = payload
                .get("options")
                .and_then(|o| o.get("chunking"))
                .and_then(|c| serde_json::from_value(c.clone()).ok())
                .unwrap_or_default();

            let bp_config = payload
                .get("options")
                .and_then(|o| o.get("backpressure"))
                .and_then(|b| serde_json::from_value(b.clone()).ok());

            let opts = IndexOptions {
                max_concurrency: payload
                    .get("options")
                    .and_then(|o| o.get("max_concurrency"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(ctx.config.max_concurrency),
                explicit_files: None,
                extract_imports: payload
                    .get("options")
                    .and_then(|o| o.get("extract_imports"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                extract_calls: payload
                    .get("options")
                    .and_then(|o| o.get("extract_calls"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                chunking: chunking_opts,
                backpressure: bp_config,
            };

            thread::spawn(move || {
                // Wait briefly for indexer to create and register its monitor in global context
                std::thread::sleep(std::time::Duration::from_millis(50));

                let bp_monitor = ctx
                    .backpressure_monitors
                    .get(&job_id)
                    .map(|entry| Arc::clone(entry.value()));

                let ev_start = Event {
                    protocol_version: "1.0.0".into(),
                    r#type: "event".into(),
                    event: "job_started".into(),
                    job_id: Some(job_id.clone()),
                    payload: Some(json!({"total_files":0})),
                };
                if let Some(ref monitor) = bp_monitor {
                    let _ = crate::infra::jsonl::emit_event_with_backpressure(monitor, ev_start);
                } else {
                    jsonl::write_event(&ev_start);
                }

                let scan_opts = crate::infra::walker::ScanOptions::new(&path);
                let _ = crate::infra::walker::emit_file_listed_events(
                    &scan_opts,
                    Some(job_id.clone()),
                    None, // Indexer will handle backpressure for file_listed
                );

                let indexer = Indexer::from_context(ctx.clone());
                let result = indexer.index_path_parallel(&path, opts, Some(job_id.clone()));
                match result {
                    Ok(result) => {
                        for chunk in &result.chunks {
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
                            if let Some(ref monitor) = bp_monitor {
                                let _ =
                                    crate::infra::jsonl::emit_event_with_backpressure(monitor, ev);
                            } else {
                                jsonl::write_event(&ev);
                            }
                        }

                        let ev_done = Event {
                            protocol_version: "1.0.0".into(),
                            r#type: "event".into(),
                            event: "job_completed".into(),
                            job_id: Some(job_id.clone()),
                            payload: Some(
                                json!({"processed": result.chunks.len(), "duration_ms": 0}),
                            ),
                        };
                        if let Some(ref monitor) = bp_monitor {
                            // Check for resume before completing the job
                            // This ensures resume event is emitted before job_completed
                            // when the queue has been cleared
                            monitor.check_and_maybe_resume();
                            let _ =
                                crate::infra::jsonl::emit_event_with_backpressure(monitor, ev_done);
                            // Cleanup: remove monitor from global map when job completes
                            ctx.backpressure_monitors.remove(&job_id);
                        } else {
                            jsonl::write_event(&ev_done);
                        }
                    }
                    Err(err) => {
                        let ev_error = Event {
                            protocol_version: "1.0.0".into(),
                            r#type: "event".into(),
                            event: "error".into(),
                            job_id: Some(job_id.clone()),
                            payload: Some(json!({
                                "code": "WALKER_ERROR",
                                "message": format!("walker failed: {:?}", err),
                                "recoverable": false
                            })),
                        };
                        if let Some(ref monitor) = bp_monitor {
                            let _ = crate::infra::jsonl::emit_event_with_backpressure(
                                monitor, ev_error,
                            );
                        } else {
                            jsonl::write_event(&ev_error);
                        }

                        let ev_done = Event {
                            protocol_version: "1.0.0".into(),
                            r#type: "event".into(),
                            event: "job_completed".into(),
                            job_id: Some(job_id.clone()),
                            payload: Some(json!({"processed": 0, "duration_ms": 0, "errors": 1})),
                        };
                        if let Some(ref monitor) = bp_monitor {
                            // Check for resume before completing the job
                            // This ensures resume event is emitted before job_completed
                            // when the queue has been cleared
                            monitor.check_and_maybe_resume();
                            let _ =
                                crate::infra::jsonl::emit_event_with_backpressure(monitor, ev_done);
                            // Cleanup: remove monitor from global map when job completes (even on error)
                            ctx.backpressure_monitors.remove(&job_id);
                        } else {
                            jsonl::write_event(&ev_done);
                        }
                    }
                }
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
            // Handle incremental_index: support use_git and git_range in payload
            let job_id = cmd
                .job_id
                .clone()
                .unwrap_or_else(|| "job-unknown".to_string());

            let payload = match cmd.payload {
                Some(p) => p,
                None => {
                    let ev = Event {
                        protocol_version: "1.0.0".into(),
                        r#type: "event".into(),
                        event: "error".into(),
                        job_id: cmd.job_id.clone(),
                        payload: Some(
                            json!({"code":"INVALID_PAYLOAD","message":"missing payload for incremental_index","recoverable":false}),
                        ),
                    };
                    jsonl::write_event(&ev);
                    return;
                }
            };

            let path = payload
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if path.is_empty() {
                let ev = Event {
                    protocol_version: "1.0.0".into(),
                    r#type: "event".into(),
                    event: "error".into(),
                    job_id: cmd.job_id.clone(),
                    payload: Some(
                        json!({"code":"INVALID_PAYLOAD","message":"missing path in payload","recoverable":false}),
                    ),
                };
                jsonl::write_event(&ev);
                return;
            }

            let use_git = payload
                .get("use_git")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // parse git_range if present
            let git_range = payload.get("git_range");
            let mut explicit_files: Option<Vec<String>> = None;

            if use_git {
                // try to obtain files from infra::git
                if let Some(range) = git_range {
                    if let (Some(from), Some(to)) = (
                        range.get("from").and_then(|v| v.as_str()),
                        range.get("to").and_then(|v| v.as_str()),
                    ) {
                        match crate::infra::git::get_git_diff_files(&path, from, to) {
                            Ok(list) => explicit_files = Some(list),
                            Err(e) => {
                                let ev = Event {
                                    protocol_version: "1.0.0".into(),
                                    r#type: "event".into(),
                                    event: "error".into(),
                                    job_id: Some(job_id.clone()),
                                    payload: Some(
                                        json!({"code":"GIT_ERROR","message":format!("git diff failed: {:?}", e),"recoverable":false}),
                                    ),
                                };
                                jsonl::write_event(&ev);
                                let ev_done = Event {
                                    protocol_version: "1.0.0".into(),
                                    r#type: "event".into(),
                                    event: "job_completed".into(),
                                    job_id: Some(job_id.clone()),
                                    payload: Some(
                                        json!({"processed": 0, "duration_ms": 0, "errors": 1}),
                                    ),
                                };
                                jsonl::write_event(&ev_done);
                                return;
                            }
                        }
                    } else {
                        // invalid git_range shape: fall back to tracked files
                        match crate::infra::git::emit_git_tracked_files(&path) {
                            Ok(list) => explicit_files = Some(list),
                            Err(e) => {
                                let ev = Event {
                                    protocol_version: "1.0.0".into(),
                                    r#type: "event".into(),
                                    event: "error".into(),
                                    job_id: Some(job_id.clone()),
                                    payload: Some(
                                        json!({"code":"GIT_ERROR","message":format!("git ls-files failed: {:?}", e),"recoverable":false}),
                                    ),
                                };
                                jsonl::write_event(&ev);
                                let ev_done = Event {
                                    protocol_version: "1.0.0".into(),
                                    r#type: "event".into(),
                                    event: "job_completed".into(),
                                    job_id: Some(job_id.clone()),
                                    payload: Some(
                                        json!({"processed": 0, "duration_ms": 0, "errors": 1}),
                                    ),
                                };
                                jsonl::write_event(&ev_done);
                                return;
                            }
                        }
                    }
                } else {
                    match crate::infra::git::emit_git_tracked_files(&path) {
                        Ok(list) => explicit_files = Some(list),
                        Err(e) => {
                            let ev = Event {
                                protocol_version: "1.0.0".into(),
                                r#type: "event".into(),
                                event: "error".into(),
                                job_id: Some(job_id.clone()),
                                payload: Some(
                                    json!({"code":"GIT_ERROR","message":format!("git ls-files failed: {:?}", e),"recoverable":false}),
                                ),
                            };
                            jsonl::write_event(&ev);
                            let ev_done = Event {
                                protocol_version: "1.0.0".into(),
                                r#type: "event".into(),
                                event: "job_completed".into(),
                                job_id: Some(job_id.clone()),
                                payload: Some(
                                    json!({"processed": 0, "duration_ms": 0, "errors": 1}),
                                ),
                            };
                            jsonl::write_event(&ev_done);
                            return;
                        }
                    }
                }
            } else {
                // not using git; check for explicit files in payload
                if let Some(files) = payload.get("files").and_then(|v| v.as_array()) {
                    let mut vec = Vec::new();
                    for f in files {
                        if let Some(s) = f.as_str() {
                            vec.push(s.to_string());
                        }
                    }
                    if !vec.is_empty() {
                        explicit_files = Some(vec);
                    }
                }
            }

            let chunking_opts = payload
                .get("options")
                .and_then(|o| o.get("chunking"))
                .and_then(|c| serde_json::from_value(c.clone()).ok())
                .unwrap_or_default();

            let bp_config = payload
                .get("options")
                .and_then(|o| o.get("backpressure"))
                .and_then(|b| serde_json::from_value(b.clone()).ok());

            let opts = IndexOptions {
                max_concurrency: payload
                    .get("options")
                    .and_then(|o| o.get("max_concurrency"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(ctx.config.max_concurrency),
                explicit_files,
                extract_imports: payload
                    .get("options")
                    .and_then(|o| o.get("extract_imports"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                extract_calls: payload
                    .get("options")
                    .and_then(|o| o.get("extract_calls"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                chunking: chunking_opts,
                backpressure: bp_config,
            };

            thread::spawn(move || {
                // Wait briefly for indexer to create and register its monitor in global context
                std::thread::sleep(std::time::Duration::from_millis(50));

                let bp_monitor = ctx
                    .backpressure_monitors
                    .get(&job_id)
                    .map(|entry| Arc::clone(entry.value()));

                let ev_start = Event {
                    protocol_version: "1.0.0".into(),
                    r#type: "event".into(),
                    event: "job_started".into(),
                    job_id: Some(job_id.clone()),
                    payload: Some(json!({"total_files":0})),
                };
                if let Some(ref monitor) = bp_monitor {
                    let _ = crate::infra::jsonl::emit_event_with_backpressure(monitor, ev_start);
                } else {
                    jsonl::write_event(&ev_start);
                }

                if let Some(ref files) = opts.explicit_files {
                    crate::infra::walker::emit_file_listed_from_records(
                        &files
                            .iter()
                            .map(|p| crate::domain::types::FileRecord {
                                path: p.clone(),
                                size: 0,
                                mtime: 0,
                                hash: "".to_string(),
                                language: None,
                            })
                            .collect::<Vec<_>>(),
                        Some(job_id.clone()),
                        None,
                    );
                } else {
                    let scan_opts = crate::infra::walker::ScanOptions::new(&path);
                    let _ = crate::infra::walker::emit_file_listed_events(
                        &scan_opts,
                        Some(job_id.clone()),
                        None,
                    );
                }

                let indexer = Indexer::from_context(ctx.clone());
                let result = indexer.index_path_parallel(&path, opts, Some(job_id.clone()));
                match result {
                    Ok(result) => {
                        for chunk in &result.chunks {
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
                            if let Some(ref monitor) = bp_monitor {
                                let _ =
                                    crate::infra::jsonl::emit_event_with_backpressure(monitor, ev);
                            } else {
                                jsonl::write_event(&ev);
                            }
                        }

                        let ev_done = Event {
                            protocol_version: "1.0.0".into(),
                            r#type: "event".into(),
                            event: "job_completed".into(),
                            job_id: Some(job_id.clone()),
                            payload: Some(
                                json!({"processed": result.chunks.len(), "duration_ms": 0}),
                            ),
                        };
                        if let Some(ref monitor) = bp_monitor {
                            // Check for resume before completing the job
                            // This ensures resume event is emitted before job_completed
                            // when the queue has been cleared
                            monitor.check_and_maybe_resume();
                            let _ =
                                crate::infra::jsonl::emit_event_with_backpressure(monitor, ev_done);
                            // Cleanup: remove monitor from global map when job completes
                            ctx.backpressure_monitors.remove(&job_id);
                        } else {
                            jsonl::write_event(&ev_done);
                        }
                    }
                    Err(err) => {
                        let ev_error = Event {
                            protocol_version: "1.0.0".into(),
                            r#type: "event".into(),
                            event: "error".into(),
                            job_id: Some(job_id.clone()),
                            payload: Some(json!({
                                "code": "WALKER_ERROR",
                                "message": format!("walker failed: {:?}", err),
                                "recoverable": false
                            })),
                        };
                        if let Some(ref monitor) = bp_monitor {
                            let _ = crate::infra::jsonl::emit_event_with_backpressure(
                                monitor, ev_error,
                            );
                        } else {
                            jsonl::write_event(&ev_error);
                        }

                        let ev_done = Event {
                            protocol_version: "1.0.0".into(),
                            r#type: "event".into(),
                            event: "job_completed".into(),
                            job_id: Some(job_id.clone()),
                            payload: Some(json!({"processed": 0, "duration_ms": 0, "errors": 1})),
                        };
                        if let Some(ref monitor) = bp_monitor {
                            // Check for resume before completing the job
                            // This ensures resume event is emitted before job_completed
                            // when the queue has been cleared
                            monitor.check_and_maybe_resume();
                            let _ =
                                crate::infra::jsonl::emit_event_with_backpressure(monitor, ev_done);
                            // Cleanup: remove monitor from global map when job completes (even on error)
                            ctx.backpressure_monitors.remove(&job_id);
                        } else {
                            jsonl::write_event(&ev_done);
                        }
                    }
                }
            });
        }
        "resume" => {
            // Real implementation: find monitor and force resume
            if let Some(job_id) = &cmd.job_id {
                if let Some(monitor_ref) = ctx.backpressure_monitors.get(job_id) {
                    let monitor = monitor_ref.value();
                    monitor.force_resume();

                    jsonl::write_event(&Event {
                        protocol_version: "1.0.0".into(),
                        r#type: "event".into(),
                        event: "ack_resume".into(),
                        job_id: Some(job_id.clone()),
                        payload: Some(json!({
                            "status": "ok",
                            "queue_size": monitor.current_queue_size(),
                            "paused": monitor.is_paused()
                        })),
                    });
                } else {
                    jsonl::write_event(&Event {
                        protocol_version: "1.0.0".into(),
                        r#type: "event".into(),
                        event: "error".into(),
                        job_id: Some(job_id.clone()),
                        payload: Some(json!({
                            "code": "JOB_NOT_FOUND",
                            "message": "no active monitor found for job_id"
                        })),
                    });
                }
            }
        }
        "ack" => {
            // Acknowledge that an event was consumed: decrement queue size
            if let Some(job_id) = &cmd.job_id {
                let count = cmd
                    .payload
                    .as_ref()
                    .and_then(|p| p.get("count"))
                    .and_then(|v| v.as_u64())
                    .map(|v| v as usize)
                    .unwrap_or(1);

                if let Some(monitor_ref) = ctx.backpressure_monitors.get(job_id) {
                    let monitor = monitor_ref.value();
                    monitor.decrement_queue_size(count);
                    monitor.check_and_maybe_resume();

                    jsonl::write_event(&Event {
                        protocol_version: "1.0.0".into(),
                        r#type: "event".into(),
                        event: "ack".into(),
                        job_id: Some(job_id.clone()),
                        payload: Some(json!({
                            "status": "ok",
                            "count": count,
                            "current_queue_size": monitor.current_queue_size(),
                            "paused": monitor.is_paused()
                        })),
                    });
                } else {
                    jsonl::write_event(&Event {
                        protocol_version: "1.0.0".into(),
                        r#type: "event".into(),
                        event: "error".into(),
                        job_id: Some(job_id.clone()),
                        payload: Some(json!({
                            "code": "JOB_NOT_FOUND",
                            "message": "no active monitor found for job_id"
                        })),
                    });
                }
            }
        }
        _ => {
            let ev = Event {
                protocol_version: "1.0.0".into(),
                r#type: "event".into(),
                event: "error".into(),
                job_id: cmd.job_id.clone(),
                payload: Some(
                    json!({"code":"INVALID_COMMAND","message":format!("unknown command: {}", cmd.command),"recoverable":false}),
                ),
            };
            jsonl::write_event(&ev);
        }
    }
}

// helper to modify command via ownership; small convenience
#[allow(dead_code)]
trait CmdExt {
    fn with_command(self, c: &str) -> Command;
}
impl CmdExt for Command {
    fn with_command(mut self, c: &str) -> Command {
        self.command = c.to_string();
        self
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;
    use crate::app::test_bootstrap::test_context;
    use crate::application::protocol::Command;
    use serde_json::json;
    use std::{thread, time::Duration};

    // These tests specifically exercise handle_command, not the dispatcher.
    // The dispatcher is tested in dispatcher_integration.rs

    #[test]
    fn handle_command_list_languages_emits_capabilities() {
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "list_languages".into(),
            seq: Some(200),
            job_id: None,
            payload: None,
        };
        handle_command(test_context(), cmd);
    }

    #[test]
    fn handle_command_index_path_valid_executes() {
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "index_path".into(),
            seq: Some(201),
            job_id: Some("job-ut-1".into()),
            payload: Some(json!({"path": ".", "options": {"max_concurrency": 1}})),
        };
        handle_command(test_context(), cmd);
        thread::sleep(Duration::from_millis(20)); // Give spawned thread a chance to start
    }

    #[test]
    fn handle_command_index_path_missing_payload_no_panic() {
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "index_path".into(),
            seq: Some(202),
            job_id: Some("job-ut-2".into()),
            payload: None,
        };
        handle_command(test_context(), cmd);
    }

    #[test]
    fn handle_command_index_path_missing_path_no_panic() {
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "index_path".into(),
            seq: Some(203),
            job_id: Some("job-ut-3".into()),
            payload: Some(json!({})),
        };
        handle_command(test_context(), cmd);
    }

    #[test]
    fn handle_command_dry_run_list_files_ack() {
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "dry_run".into(),
            seq: Some(204),
            job_id: Some("job-ut-4".into()),
            payload: None,
        };
        handle_command(test_context(), cmd);
    }

    #[test]
    fn handle_command_resume_and_incremental_alias() {
        let cmd_resume = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "resume".into(),
            seq: Some(205),
            job_id: Some("job-ut-5".into()),
            payload: None,
        };
        handle_command(test_context(), cmd_resume);

        let cmd_inc = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "incremental_index".into(),
            seq: Some(206),
            job_id: Some("job-ut-6".into()),
            payload: Some(json!({"path": "."})),
        };
        handle_command(test_context(), cmd_inc);
    }

    #[test]
    fn cmdext_with_command_replaces_command() {
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "dry_run".into(),
            seq: Some(300),
            job_id: None,
            payload: None,
        };
        let new = cmd.with_command("index_path");
        assert_eq!(new.command, "index_path");
    }
}
