#[cfg(feature = "parsing")]
mod rust_adapter {
    use super::LanguageAdapter;
    use crate::domain::parser::ParsedFile;
    use crate::domain::types::Symbol;
    use anyhow::Result;
    use tree_sitter::{Parser, Language};
    extern "C" { fn tree_sitter_rust() -> Language; }

    pub struct RustAdapter;

    impl RustAdapter {
        pub fn new() -> Self { RustAdapter }
    }

    impl LanguageAdapter for RustAdapter {
        fn parse_source(&self, source: &str) -> Result<ParsedFile> {
            let mut parser = Parser::new();
            unsafe { parser.set_language(tree_sitter_rust()).unwrap(); }
            let tree = parser.parse(source, None).ok_or_else(|| anyhow::anyhow!("parse failed"))?;
            Ok(ParsedFile { language: "rust".to_string(), source_len: source.len() })
        }
        fn extract_symbols(&self, _parsed: &ParsedFile) -> Result<Vec<Symbol>> {
            // placeholder: real implementation would walk the tree and extract symbols
            Ok(vec![])
        }
        fn box_clone(&self) -> Box<dyn LanguageAdapter> { Box::new(RustAdapter::new()) }
    }

    // register at init when feature enabled
    pub fn register() {
        crate::adapters::register_adapter("rust", Box::new(RustAdapter::new()));
    }
}

#[cfg(not(feature = "parsing"))]
mod rust_adapter {
    // stub implementation when parsing feature not enabled
}

pub use rust_adapter::*;
