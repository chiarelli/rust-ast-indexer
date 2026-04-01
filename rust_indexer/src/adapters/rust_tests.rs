#[cfg(all(test, feature = "parsing"))]
mod tests {
    use super::*;
    use crate::adapters::RustAdapter;

    #[test]
    fn rust_adapter_parses_simple_fn() {
        let adapter = RustAdapter::new();
        let src = "fn hello(name: &str) -> String { format!(\"hi {}\", name) }";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        assert_eq!(parsed.language, "rust");
        assert_eq!(parsed.source_len, src.len());

        let syms = adapter.extract_symbols(&parsed).expect("extract_symbols should run");
        // placeholder implementation currently returns empty vector
        assert!(syms.is_empty());
    }

    #[test]
    fn rust_adapter_handles_empty_source() {
        let adapter = RustAdapter::new();
        let src = "";
        let parsed = adapter.parse_source(src).expect("parse should succeed on empty");
        assert_eq!(parsed.language, "rust");
        assert_eq!(parsed.source_len, 0);
    }
}
