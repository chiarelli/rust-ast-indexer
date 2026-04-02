// ParserPool using LanguageAdapter instances per language
// Thread-safe pool providing adapters for parsing

use std::sync::Arc;
use crate::adapters::LanguageAdapter;
use dashmap::DashMap;

pub struct ParserPool {
    adapters: DashMap<String, Arc<dyn LanguageAdapter>>,
}

impl ParserPool {
    pub fn new() -> Self {
        Self {
            adapters: DashMap::new(),
        }
    }

    /// Register a language adapter into the pool
    pub fn register(&self, lang: &str, adapter: Arc<dyn LanguageAdapter>) {
        self.adapters.insert(lang.to_string(), adapter);
    }

    /// Get an adapter for the given language
    pub fn get(&self, lang: &str) -> Option<Arc<dyn LanguageAdapter>> {
        self.adapters.get(lang).map(|e| Arc::clone(e.value()))
    }

    /// List supported languages
    pub fn languages(&self) -> Vec<String> {
        self.adapters.iter().map(|e| e.key().clone()).collect()
    }
}

#[cfg(all(test))]
mod tests {
    use super::*;
    use crate::domain::parser::ParsedFile;
    use crate::domain::types::Symbol;
    use anyhow::Result;

    struct MockAdapter;
    impl LanguageAdapter for MockAdapter {
        fn parse_source(&self, source: &str) -> Result<ParsedFile> {
            Ok(ParsedFile { language: "mock".to_string(), source_len: source.len(), source: source.to_string() })
        }
        fn extract_symbols(&self, _parsed: &ParsedFile) -> Result<Vec<Symbol>> {
            Ok(vec![])
        }
        fn box_clone(&self) -> Box<dyn LanguageAdapter> { Box::new(MockAdapter) }
    }

    #[test]
    fn pool_register_and_get() {
        let pool = ParserPool::new();
        pool.register("test", Arc::new(MockAdapter));
        let adapter = pool.get("test");
        assert!(adapter.is_some());
        let parsed = adapter.unwrap().parse_source("fn test() {}").unwrap();
        assert_eq!(parsed.language, "mock");
    }

    #[test]
    fn pool_get_missing_lang_returns_none() {
        let pool = ParserPool::new();
        assert!(pool.get("nonexistent").is_none());
    }

    #[test]
    fn pool_languages_lists_registered() {
        let pool = ParserPool::new();
        pool.register("rust", Arc::new(MockAdapter));
        pool.register("java", Arc::new(MockAdapter));
        let mut langs = pool.languages();
        langs.sort();
        assert_eq!(langs, vec!["java", "rust"]);
    }
}

