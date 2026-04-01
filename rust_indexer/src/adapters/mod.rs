use crate::domain::types::{ParsedFile, Symbol};
pub mod rust;
use anyhow::Result;

pub trait LanguageAdapter: Send + Sync + 'static {
    fn parse_source(&self, source: &str) -> Result<ParsedFile>;
    fn extract_symbols(&self, parsed: &ParsedFile) -> Result<Vec<Symbol>>;
    fn box_clone(&self) -> Box<dyn LanguageAdapter>;
}

// Adapter registry (compat shim)
use std::sync::Arc;
use crate::app::bootstrap::Registry;

/// Temporary compatibility helpers that delegate to a provided Registry in ApplicationContext.
/// Migration note: replace usages with ctx.registry.get(...)
pub fn register_adapter_compat(registry: &Registry, lang: &str, adapter: Arc<dyn LanguageAdapter>) {
    registry.register(lang, adapter);
}

pub fn get_adapter_compat(registry: &Registry, lang: &str) -> Option<Arc<dyn LanguageAdapter>> {
    registry.get(lang)
}

// Provide a helper macro to register adapters at init time via a Registry reference
#[macro_export]
macro_rules! register_language_adapter {
    ($registry:expr, $lang:expr, $adapter:expr) => {
        crate::adapters::register_adapter_compat($registry, $lang, std::sync::Arc::new($adapter));
    };
}
