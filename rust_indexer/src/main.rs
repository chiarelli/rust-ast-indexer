// Binary entrypoint
use rust_indexer::app::bootstrap::Config;
use rust_indexer::app::bootstrap::init_context;
use rust_indexer::run_cli;

fn main() {
    let cfg = Config::load();
    let ctx = init_context(cfg);
    run_cli(ctx);
}
