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
        // Sem path no ParsedFile (parse_source direto), o adapter cai no
        // placeholder vazio — o caminho real entra via `parsed.path` no
        // pipeline (indexer.rs: parsed.path = file.path.clone()).
        assert_eq!(e.from_file, "");
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

    /// O caminho real do arquivo (`parsed.path`, preenchido pelo pipeline em
    /// `indexer.rs`) tem de aparecer em `from_file`/`caller_symbol_id` — é o
    /// que o consumidor (memtier) usa para casar a aresta com o chunk.
    #[test]
    fn go_adapter_uses_real_path_when_parsed_path_is_set() {
        let adapter = GoAdapter::new();
        let src = "package p\n\nimport \"fmt\"\n\nfunc hello() { fmt.Println(\"x\") }\n";
        let mut parsed = adapter.parse_source(src).expect("parse should succeed");
        parsed.path = "internal/pkg/file.go".to_string();

        let edges = adapter.extract_imports(&parsed).expect("extract_imports should run");
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].from_file, "internal/pkg/file.go");
        assert!(!edges[0].id.contains("<source>"), "id não pode ter placeholder: {}", edges[0].id);

        let calls = adapter.extract_calls(&parsed).expect("extract_calls should run");
        assert!(!calls.is_empty());
        assert_eq!(
            calls[0].caller_symbol_id.as_deref(),
            Some("internal/pkg/file.go:hello")
        );

        let syms = adapter.extract_symbols(&parsed).expect("extract_symbols should run");
        assert!(syms.iter().all(|s| s.file_path == "internal/pkg/file.go"));
        assert!(syms.iter().all(|s| !s.id.contains("<source>")));
    }
}