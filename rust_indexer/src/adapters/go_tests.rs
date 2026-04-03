#[cfg(all(test, feature = "parsing"))]
mod tests {
    use crate::adapters::{LanguageAdapter, go::GoAdapter};

    #[test]
    fn go_adapter_parses_simple_fn() {
        let adapter = GoAdapter::new();
        let src = "func hello(name string) string { return \"hi \" + name }";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        assert_eq!(parsed.language, "go");
        assert_eq!(parsed.source_len, src.len());

        let syms = adapter.extract_symbols(&parsed).expect("extract_symbols should run");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "hello");
        assert_eq!(syms[0].kind, "function");
    }

    #[test]
    fn go_adapter_handles_empty_source() {
        let adapter = GoAdapter::new();
        let src = "";
        let parsed = adapter.parse_source(src).expect("parse should succeed on empty");
        assert_eq!(parsed.language, "go");
        assert_eq!(parsed.source_len, 0);
    }

    #[test]
    fn go_adapter_extracts_import() {
        let adapter = GoAdapter::new();
        let src = "import \"fmt\"";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let imports: Vec<_> = syms.iter().filter(|s| s.kind == "import").collect();
        assert_eq!(imports.len(), 1);
    }

    #[test]
    fn go_adapter_extracts_import_edges() {
        let adapter = GoAdapter::new();
        let src = "import \"fmt\"";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let edges = adapter.extract_imports(&parsed).expect("extract_imports should run");
        assert_eq!(edges.len(), 1);
        let e = &edges[0];
        assert!(e.from_file.contains("<source>") || e.from_file == "<source>");
        assert_eq!(e.to_module, "import \"fmt\"");
        assert_eq!(e.import_kind, "named");
        assert!(!e.resolved);
    }

    #[test]
    fn go_adapter_extracts_call_edges() {
        let adapter = GoAdapter::new();
        let src = r#"
package main
import "fmt"
func main() {
    fmt.Println("hello")
    len([]int{})
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let edges = adapter.extract_calls(&parsed).expect("extract_calls should run");
        // Should have at least 2 calls: fmt.Println and len
        assert!(edges.len() >= 2);
        // Check that we have Println call
        let println_call = edges.iter().find(|e| e.callee_name == "fmt.Println");
        assert!(println_call.is_some());
        // Check that we have len call
        let len_call = edges.iter().find(|e| e.callee_name == "len");
        assert!(len_call.is_some());
    }

    #[test]
    fn go_adapter_box_clone() {
        let adapter = GoAdapter::new();
        let cloned = adapter.box_clone();
        let src = "func test() {}";
        let parsed = cloned.parse_source(src).expect("clone should work");
        let syms = cloned.extract_symbols(&parsed).unwrap();
        assert_eq!(syms.len(), 1);
    }

    #[test]
    fn go_adapter_source_only_whitespace() {
        let adapter = GoAdapter::new();
        let src = "   \n\n  \t  ";
        let parsed = adapter.parse_source(src).expect("should not crash on whitespace");
        assert_eq!(parsed.language, "go");
    }
}