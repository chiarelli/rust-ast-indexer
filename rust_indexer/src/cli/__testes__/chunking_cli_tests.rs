#[cfg(test)]
mod tests {
    use crate::app::test_bootstrap::test_context;
    use crate::application::protocol::Command;
    use crate::cli::dispatcher;
    use crate::cli::handle_command;
    use serde_json::json;
    use std::{thread, time::Duration};

    #[test]
    fn handle_command_index_path_with_chunking_options() {
        // Test that chunking options are correctly parsed from CLI payload
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "index_path".into(),
            seq: Some(300),
            job_id: Some("job-chunking-1".into()),
            payload: Some(json!({
                "path": ".",
                "options": {
                    "max_concurrency": 2,
                    "chunking": {
                        "strategy": "semantic",
                        "max_lines": 100,
                        "overlap_lines": 2,
                        "include_context": true,
                        "token_counting": false
                    }
                }
            })),
        };

        // dispatcher should not handle index_path (returns false) so CLI processes it
        assert!(!dispatcher::dispatch_cmd(&cmd));

        // Process the command
        handle_command(test_context(), cmd);
        thread::sleep(Duration::from_millis(50)); // Give spawned thread time to execute
    }

    #[test]
    fn handle_command_index_path_with_symbol_boundary_strategy() {
        // Test SymbolBoundary strategy
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "index_path".into(),
            seq: Some(301),
            job_id: Some("job-chunking-2".into()),
            payload: Some(json!({
                "path": ".",
                "options": {
                    "chunking": {
                        "strategy": "symbol_boundary",
                        "max_lines": 50,
                        "overlap_lines": 0,
                        "include_context": false,
                        "token_counting": true
                    }
                }
            })),
        };

        assert!(!dispatcher::dispatch_cmd(&cmd));
        handle_command(test_context(), cmd);
        thread::sleep(Duration::from_millis(50));
    }

    #[test]
    fn handle_command_index_path_with_line_limited_strategy() {
        // Test LineLimited strategy
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "index_path".into(),
            seq: Some(302),
            job_id: Some("job-chunking-3".into()),
            payload: Some(json!({
                "path": ".",
                "options": {
                    "chunking": {
                        "strategy": "line_limited",
                        "max_lines": 30,
                        "overlap_lines": 3,
                        "include_context": true,
                        "token_counting": false
                    }
                }
            })),
        };

        assert!(!dispatcher::dispatch_cmd(&cmd));
        handle_command(test_context(), cmd);
        thread::sleep(Duration::from_millis(50));
    }

    #[test]
    fn handle_command_incremental_index_with_chunking_options() {
        // Test that incremental_index also accepts chunking options
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "incremental_index".into(),
            seq: Some(303),
            job_id: Some("job-chunking-4".into()),
            payload: Some(json!({
                "path": ".",
                "files": ["src/main.rs"],
                "options": {
                    "chunking": {
                        "strategy": "semantic",
                        "max_lines": 150,
                        "overlap_lines": 1,
                        "include_context": true,
                        "token_counting": false
                    }
                }
            })),
        };

        assert!(!dispatcher::dispatch_cmd(&cmd));
        handle_command(test_context(), cmd);
        thread::sleep(Duration::from_millis(50));
    }

    #[test]
    fn handle_command_index_path_chunking_options_use_defaults_when_missing() {
        // Test that missing chunking options use defaults
        let cmd = Command {
            protocol_version: "1.0.0".into(),
            r#type: "command".into(),
            command: "index_path".into(),
            seq: Some(304),
            job_id: Some("job-chunking-5".into()),
            payload: Some(json!({
                "path": ".",
                "options": {
                    "max_concurrency": 4
                    // note: no chunking options provided
                }
            })),
        };

        assert!(!dispatcher::dispatch_cmd(&cmd));
        handle_command(test_context(), cmd);
        thread::sleep(Duration::from_millis(50));
    }
}
