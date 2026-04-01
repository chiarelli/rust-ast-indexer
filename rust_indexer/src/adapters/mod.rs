use crate::domain::types::{ParsedFile, Symbol};
pub mod rust;
use anyhow::Result;

pub trait LanguageAdapter: Send + Sync + 'static {
    fn parse_source(&self, source: &str) -> Result<ParsedFile>;
    fn extract_symbols(&self, parsed: &ParsedFile) -> Result<Vec<Symbol>>;
    fn box_clone(&self) -> Box<dyn LanguageAdapter>;
}

// Adapter registry (simple)
use std::collections::HashMap;
use std::sync::RwLock;

use lazy_static::lazy_static;

lazy_static! {
    static ref ADAPTERS: RwLock<HashMap<String, Box<dyn LanguageAdapter>>> = RwLock::new(HashMap::new());
}

pub fn register_adapter(lang: &str, adapter: Box<dyn LanguageAdapter>) {
    let mut m = ADAPTERS.write().unwrap();
    m.insert(lang.to_string(), adapter);
}

pub fn get_adapter(lang: &str) -> Option<Box<dyn LanguageAdapter>> {
    let m = ADAPTERS.read().unwrap();
    m.get(lang).map(|a| a.box_clone())
}

// Provide a helper macro to register adapters at init time
#[macro_export]
macro_rules! register_language_adapter {
    ($lang:expr, $adapter:expr) => {
        crate::adapters::register_adapter($lang, Box::new($adapter));
    };
}
