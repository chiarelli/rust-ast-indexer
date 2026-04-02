use std::sync::Arc;

use crate::app::bootstrap::{ApplicationContext, Config, init_context};

pub fn test_context() -> Arc<ApplicationContext> {
    let cfg = Config { max_concurrency: 1, max_queue_size: 10 };
    init_context(cfg)
}
