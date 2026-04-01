use std::sync::Arc;

use crate::infra::parser_pool::ParserPool;
use dashmap::DashMap;


#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(serde_json::Error),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "failed to read config file: {}", e),
            ConfigError::Parse(e) => write!(f, "failed to parse config: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}

impl From<serde_json::Error> for ConfigError {
    fn from(e: serde_json::Error) -> Self {
        ConfigError::Parse(e)
    }
}

#[derive(Debug)]
pub struct Config {
    pub max_concurrency: usize,
    pub max_queue_size: usize,
}

impl Config {
    pub fn new(max_concurrency: usize, max_queue_size: usize) -> Self {
        Self { max_concurrency, max_queue_size }
    }

    #[cfg(feature = "parsing")]
    pub fn from_file(path: &str) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    /// Loads config by checking environment variables first, then falling back to defaults.
    pub fn from_env() -> Self {
        let max_concurrency = std::env::var("MAX_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or_else(num_cpus::get);

        let max_queue_size = std::env::var("MAX_QUEUE_SIZE")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(100);

        Self { max_concurrency, max_queue_size }
    }

    /// Attempts to load from a config file; falls back to environment variables.
    pub fn load() -> Self {
        // try common config file names in current directory
        #[allow(unused_variables)]
        for path in ["rust_indexer.json", ".rust_indexer.json", "config.json"] {
            #[cfg(feature = "parsing")]
            if let Ok(config) = Self::from_file(path) {
                return config;
            }
        }
        Self::from_env()
    }
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

    pub fn list_languages(&self) -> Vec<String> {
        self.inner.iter().map(|entry| entry.key().clone()).collect()
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

    #[test]
    fn config_new_creates_instance() {
        let cfg = Config::new(4, 50);
        assert_eq!(cfg.max_concurrency, 4);
        assert_eq!(cfg.max_queue_size, 50);
    }

    #[test]
    fn config_from_env_defaults_without_vars() {
        let cfg = Config::from_env();
        assert!(cfg.max_concurrency > 0);
        assert!(cfg.max_queue_size > 0);
    }

    #[test]
    fn config_from_env_respects_vars() {
        std::env::set_var("MAX_CONCURRENCY", "8");
        std::env::set_var("MAX_QUEUE_SIZE", "200");
        let cfg = Config::from_env();
        assert_eq!(cfg.max_concurrency, 8);
        assert_eq!(cfg.max_queue_size, 200);
        std::env::remove_var("MAX_CONCURRENCY");
        std::env::remove_var("MAX_QUEUE_SIZE");
    }

    #[test]
    fn register_and_list_languages() {
        use crate::adapters::LanguageAdapter;
        use crate::domain::parser::ParsedFile;
        use anyhow::Result;

        struct StubAdapter;
        impl LanguageAdapter for StubAdapter {
            fn parse_source(&self, _source: &str) -> Result<ParsedFile> {
                Ok(ParsedFile { language: "stub".to_string(), source_len: 0 })
            }
            fn extract_symbols(&self, _parsed: &ParsedFile) -> Result<Vec<crate::domain::types::Symbol>> {
                Ok(vec![])
            }
            fn box_clone(&self) -> Box<dyn LanguageAdapter> {
                Box::new(StubAdapter)
            }
        }

        let reg = Registry::new();
        assert!(reg.list_languages().is_empty());
        reg.register("test", Arc::new(StubAdapter));
        let mut langs = reg.list_languages();
        langs.sort();
        assert_eq!(langs, vec!["test"]);
        assert!(reg.get("test").is_some());
    }
}
