use std::sync::Arc;

use crate::infra::parser_pool::ParserPool;

use dashmap::DashMap;
use anyhow::Result;

use crate::adapters::LanguageAdapter;

pub struct Config {
    pub max_concurrency: usize,
    pub max_queue_size: usize,
}

pub struct Registry {
    inner: DashMap<String, Arc<dyn LanguageAdapter>>,
}

impl Registry {
    pub fn new() -> Self {
        Self { inner: DashMap::new() }
    }

    pub fn register(&self, lang: &str, adapter: Arc<dyn LanguageAdapter>) {
        self.inner.insert(lang.to_string(), adapter);
    }

    pub fn get(&self, lang: &str) -> Option<Arc<dyn LanguageAdapter>> {
        self.inner.get(lang).map(|v| v.value().clone())
    }
}

pub struct ApplicationContext {
    pub registry: Arc<Registry>,
    pub parser_pool: Arc<ParserPool>,
    pub config: Config,
}

pub fn init_context(config: Config) -> Arc<ApplicationContext> {
    let registry = Arc::new(Registry::new());
    let parser_pool = Arc::new(ParserPool::new(config.max_concurrency));

    Arc::new(ApplicationContext { registry, parser_pool, config })
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
    }
}
