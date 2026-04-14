#[derive(Debug, Clone, Default)]
pub struct CliArgs {
    pub mcp_mode: bool,
    pub quiet: bool,
}

impl CliArgs {
    pub fn parse_from(args: &[String]) -> Self {
        let mut mcp_mode = false;
        let mut quiet = false;

        for arg in args.iter().skip(1) {
            match arg.as_str() {
                "--mcp" | "-m" => mcp_mode = true,
                "--quiet" | "-q" => quiet = true,
                _ => {}
            }
        }

        Self { mcp_mode, quiet }
    }
}

pub fn run_mcp_mode() {
    use crate::infra::mcp_adapter::McpAdapter;
    use serde_json::json;
    use std::io::{self, BufRead};

    let adapter = McpAdapter::new();

    adapter.register_handler("list_languages", |_| {
        Ok(json!({
            "languages": ["rust", "go", "python", "typescript", "javascript", "java"]
        }))
    });

    adapter.register_handler("index_path", |_params| {
        Ok(json!({ "job_id": "async", "status": "started" }))
    });

    adapter.register_handler("stop", |_| Ok(json!({ "status": "stopped" })));

    adapter.register_handler("status", |_| {
        Ok(json!({
            "status": "ready",
            "version": "0.1.0"
        }))
    });

    let stdin = io::stdin();
    for line in stdin.lock().lines().map_while(Result::ok) {
        let l = line.trim();
        if l.is_empty() {
            continue;
        }

        match adapter.parse_request(l) {
            Ok(req) => {
                let resp = adapter.handle_request(req);
                println!("{}", resp.to_value());
            }
            Err(e) => {
                let resp = crate::infra::mcp_adapter::JsonRpcResponse::error(None, -32600, &e);
                println!("{}", resp.to_value());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_args_parse_mcp_flag() {
        let args = CliArgs::parse_from(&["indexer".to_string(), "--mcp".to_string()]);
        assert!(args.mcp_mode);
    }

    #[test]
    fn cli_args_default_no_mcp() {
        let args = CliArgs::parse_from(&["indexer".to_string()]);
        assert!(!args.mcp_mode);
    }

    #[test]
    fn cli_args_mcp_short_flag() {
        let args = CliArgs::parse_from(&["indexer".to_string(), "-m".to_string()]);
        assert!(args.mcp_mode);
    }

    #[test]
    fn cli_args_mixed_flags() {
        let args = CliArgs::parse_from(&[
            "indexer".to_string(),
            "-m".to_string(),
            "--quiet".to_string(),
        ]);
        assert!(args.mcp_mode);
        assert!(args.quiet);
    }

    #[test]
    fn cli_args_unknown_flag_ignored() {
        let args = CliArgs::parse_from(&["indexer".to_string(), "--unknown".to_string()]);
        assert!(!args.mcp_mode);
    }
}
