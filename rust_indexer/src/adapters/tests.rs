#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::LanguageAdapter;
    use crate::domain::parser::ParsedFile;
    use crate::domain::types::Symbol;
    use anyhow::Result;

    #[derive(Clone)]
    struct DummyAdapter;

    impl LanguageAdapter for DummyAdapter {
        fn parse_source(&self, source: &str) -> Result<ParsedFile> {
            Ok(ParsedFile { language: "dummy".to_string(), source_len: source.len() })
        }
        fn extract_symbols(&self, _parsed: &ParsedFile) -> Result<Vec<Symbol>> {
            Ok(vec![Symbol { id: "s1".to_string(), name: "foo".to_string(), kind: "function".to_string(), scope: None, file_path: "a.rs".to_string(), start_line: 1, end_line: 3, signature: None }])
        }
        fn box_clone(&self) -> Box<dyn LanguageAdapter> { Box::new(self.clone()) }
    }

    #[test]
    fn adapter_contract_parse_and_extract() {
        let adapter = DummyAdapter;
        let parsed = adapter.parse_source("fn foo() {}").unwrap();
        assert_eq!(parsed.source_len, 12);
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "foo");
    }

    #[test]
    fn registry_register_and_get() {
        let ctx = crate::app::test_bootstrap::test_context();
        ctx.registry.register("dummy", std::sync::Arc::new(DummyAdapter));
        let got = ctx.registry.get("dummy").expect("adapter should be present");
        let parsed = got.parse_source("x").unwrap();
        assert_eq!(parsed.language, "dummy");
    }
}
