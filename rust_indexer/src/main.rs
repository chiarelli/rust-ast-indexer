// Binary entrypoint
use std::sync::Arc;
use rust_indexer::app::bootstrap::Config;
use rust_indexer::app::bootstrap::init_context;
use rust_indexer::run_cli;

fn main() {
    let cfg = Config { max_concurrency: num_cpus::get(), max_queue_size: 100 };
    let ctx = init_context(cfg);
    run_cli(ctx);
}
