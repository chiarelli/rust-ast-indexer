use std::sync::Arc;

use crate::infra::parser_pool::ParserPool;
use dashmap::DashMap;

pub struct Config {
    pub max_concurrency: usize,
    pub max_queue_size: usize,
}

pub struct Registry {
    inner: DashMap<String, Arc<dyn crate::adapters::LanguageAdapter>>,
}

impl Default for Registry {
    fn default() -> Self {
        Self { inner: DashMap::new() }
    }
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, lang: &str, adapter: Arc<dyn crate::adapters::LanguageAdapter>) {
        self.inner.insert(lang.to_string(), adapter);
    }

    pub fn get(&self, lang: &str) -> Option<Arc<dyn crate::adapters::LanguageAdapter>> {
        self.inner.get(lang).as_ref().map(|entry| Arc::clone(entry.value()))
    }
}

/// Placeholder for metrics collection; currently unused.
/// When real metrics integration arrives this struct
/// will implement that interface (prometheus/otel).
pub struct Metrics {}

/// Placeholder for logging; currently unused.
/// Replaces ad-hoc println!/eprintln in favor of structured output.
pub struct Logger {}

pub struct ApplicationContext {
    pub registry: Arc<Registry>,
    pub parser_pool: Arc<ParserPool>,
    pub config: Config,
    pub metrics: Option<Arc<Metrics>>,
    pub logger: Option<Arc<Logger>>,
}

pub fn init_context(config: Config) -> Arc<ApplicationContext> {
    let registry = Arc::new(Registry::new());
    let parser_pool = Arc::new(ParserPool::new(config.max_concurrency));

    // register built-in adapters into the registry
    #[cfg(feature = "parsing")]
    {
        // rust adapter registers itself into the provided registry
        crate::adapters::rust::register_to(&registry);
    }

    Arc::new(ApplicationContext { registry, parser_pool, config, metrics: None, logger: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_context_returns_context() {
        let cfg = Config { max_concurrency: 2, max_queue_size: 10 };
        let ctx = init_context(cfg);
        assert!(Arc::strong_count(&ctx) >= 1);
        assert!(ctx.registry.get("nope").is_none());
        assert!(ctx.metrics.is_none());
        assert!(ctx.logger.is_none());
    }
}
