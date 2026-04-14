// Binary entrypoint
use rust_indexer::app::bootstrap::init_context;
use rust_indexer::app::bootstrap::Config;
use rust_indexer::cli::args::CliArgs;
use rust_indexer::run_cli;
use std::env;

fn main() {
    let args = CliArgs::parse_from(&env::args().collect::<Vec<_>>());

    if args.mcp_mode {
        rust_indexer::cli::args::run_mcp_mode();
        return;
    }

    let cfg = Config::load();
    let ctx = init_context(cfg);
    run_cli(ctx);
}
