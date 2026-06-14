#[cfg(all(test, feature = "parsing"))]
mod tests {
    use crate::adapters::{LanguageAdapter, python::PythonAdapter};

    #[test]
    fn python_adapter_parses_simple_fn() {
        let adapter = PythonAdapter::new();
        let src = "def hello(name: str) -> str:\n    return f\"hi {name}\"";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        assert_eq!(parsed.language, "python");
        assert_eq!(parsed.source_len, src.len());

        let syms = adapter.extract_symbols(&parsed).expect("extract_symbols should run");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "hello");
        assert_eq!(syms[0].kind, "function");
    }

    #[test]
    fn python_adapter_handles_empty_source() {
        let adapter = PythonAdapter::new();
        let src = "";
        let parsed = adapter
            .parse_source(src)
            .expect("parse should succeed on empty");
        assert_eq!(parsed.language, "python");
        assert_eq!(parsed.source_len, 0);
    }

    #[test]
    fn python_adapter_extracts_class() {
        let adapter = PythonAdapter::new();
        let src = r#"
class UserService:
    def get_user(self, id: int) -> dict:
        return {"id": id}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let classes: Vec<_> = syms.iter().filter(|s| s.kind == "class").collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name, "UserService");
    }

    #[test]
    fn python_adapter_extracts_method() {
        let adapter = PythonAdapter::new();
        let src = r#"
class Calc:
    def add(self, a: int, b: int) -> int:
        return a + b
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let methods: Vec<_> = syms.iter().filter(|s| s.kind == "function").collect();
        assert!(
            methods.iter().any(|s| s.name == "add"),
            "Expected function 'add', got: {:?}",
            methods.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn python_adapter_extracts_import() {
        let adapter = PythonAdapter::new();
        let src = "import os\nfrom sys import path";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let imports: Vec<_> = syms.iter().filter(|s| s.kind == "import").collect();
        assert_eq!(imports.len(), 2);
    }

    #[test]
    fn python_adapter_extracts_variable() {
        let adapter = PythonAdapter::new();
        let src = "MAX_RETRIES = 3\nPI = 3.14";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let vars: Vec<_> = syms.iter().filter(|s| s.kind == "variable").collect();
        assert_eq!(vars.len(), 2);
        assert!(vars.iter().any(|s| s.name == "MAX_RETRIES"));
    }

    #[test]
    fn python_adapter_decorated_function() {
        let adapter = PythonAdapter::new();
        let src = r#"
@cache
def fibonacci(n: int) -> int:
    if n <= 1:
        return n
    return fibonacci(n-1) + fibonacci(n-2)
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let functions: Vec<_> = syms.iter().filter(|s| s.kind == "function").collect();
        assert!(
            functions.iter().any(|s| s.name == "fibonacci"),
            "Expected function 'fibonacci', got: {:?}",
            functions.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
    }

    #[test]
    fn python_adapter_extracts_import_edges() {
        let adapter = PythonAdapter::new();
        let src = "import json\nfrom collections import defaultdict";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let edges = adapter.extract_imports(&parsed).expect("extract_imports should run");
        assert_eq!(edges.len(), 2);
        assert!(!edges[0].resolved);
    }

    #[test]
    fn python_adapter_symbol_has_signature() {
        let adapter = PythonAdapter::new();
        let src = "def compute(a: int, b: int) -> int:\n    return a + b";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert!(syms[0].signature.is_some());
        assert!(syms[0]
            .signature
            .as_ref()
            .unwrap()
            .contains("compute"));
    }

    #[test]
    fn python_adapter_box_clone() {
        let adapter = PythonAdapter::new();
        let cloned = adapter.box_clone();
        let src = "def test(): pass";
        let parsed = cloned.parse_source(src).expect("clone should work");
        let syms = cloned.extract_symbols(&parsed).unwrap();
        assert_eq!(syms.len(), 1);
    }

    #[test]
    fn python_adapter_source_only_whitespace() {
        let adapter = PythonAdapter::new();
        let src = "   \n\n  \t  ";
        let parsed = adapter
            .parse_source(src)
            .expect("should not crash on whitespace");
        assert_eq!(parsed.language, "python");
    }

    #[test]
    fn python_adapter_extracts_call_edges() {
        let adapter = PythonAdapter::new();
        let src = r#"
def process():
    result = format("hello {}", "world")
    print(result)
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let edges = adapter.extract_calls(&parsed).expect("extract_calls should run");
        assert!(
            edges.len() >= 2,
            "Expected >= 2 call edges, got: {}",
            edges.len()
        );
    }

    #[test]
    fn python_adapter_multiple_symbols() {
        let adapter = PythonAdapter::new();
        let src = r#"
import sys
import os

CONFIG_FILE = "config.yaml"

class Config:
    port = 8080

def start():
    print("starting")

def stop():
    print("stopping")
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let kinds: Vec<&str> = syms.iter().map(|s| s.kind.as_str()).collect();
        assert!(kinds.contains(&"import"), "kinds: {:?}", kinds);
        assert!(kinds.contains(&"variable"), "kinds: {:?}", kinds);
        assert!(kinds.contains(&"class"), "kinds: {:?}", kinds);
        assert!(kinds.contains(&"function"), "kinds: {:?}", kinds);
        assert!(
            syms.len() >= 6,
            "Expected >= 6 symbols, got: {}",
            syms.len()
        );
    }

    #[test]
    fn python_adapter_symbol_has_line_range() {
        let adapter = PythonAdapter::new();
        let src = "def test_fn():\n    pass";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].start_line, 0);
        assert!(syms[0].end_line >= 1);
    }
}
