#[cfg(all(test, feature = "parsing"))]
mod tests {
    use crate::adapters::{LanguageAdapter, typescript::TypeScriptAdapter};

    #[test]
    fn ts_adapter_parses_simple_fn() {
        let adapter = TypeScriptAdapter::new();
        let src = "function hello(name: string): string { return `hi ${name}`; }";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        assert_eq!(parsed.language, "typescript");
        assert_eq!(parsed.source_len, src.len());

        let syms = adapter.extract_symbols(&parsed).expect("extract_symbols should run");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "hello");
        assert_eq!(syms[0].kind, "function");
    }

    #[test]
    fn ts_adapter_handles_empty_source() {
        let adapter = TypeScriptAdapter::new();
        let src = "";
        let parsed = adapter.parse_source(src).expect("parse should succeed on empty");
        assert_eq!(parsed.language, "typescript");
        assert_eq!(parsed.source_len, 0);
    }

    #[test]
    fn ts_adapter_extracts_class() {
        let adapter = TypeScriptAdapter::new();
        let src = r#"
class UserManager {
    constructor() {}
    getName() { return this.name; }
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let classes: Vec<_> = syms.iter().filter(|s| s.kind == "class").collect();
        assert_eq!(classes.len(), 1, "Expected 1 class, got {:?}, symbols: {:?}", classes.len(), syms.iter().map(|s| s.kind.clone()).collect::<Vec<_>>());
        assert_eq!(classes[0].name, "UserManager");
    }

    #[test]
    fn ts_adapter_extracts_import() {
        let adapter = TypeScriptAdapter::new();
        let src = r#"
import { useState, useEffect } from "react";
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let imports: Vec<_> = syms.iter().filter(|s| s.kind == "import").collect();
        assert!(imports.len() >= 1, "Expected imports, got: {:?}", syms);
    }

    #[test]
    fn ts_adapter_extracts_export() {
        let adapter = TypeScriptAdapter::new();
        let src = r#"
export function getVersion() { return "1.0.0"; }
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let exports: Vec<_> = syms.iter().filter(|s| s.kind == "export").collect();
        assert!(exports.len() >= 1, "Expected exports, got: {:?}", syms);
    }

    #[test]
    fn ts_adapter_extracts_variable() {
        let adapter = TypeScriptAdapter::new();
        let src = "const MAX_RETRIES = 3;";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let vars: Vec<_> = syms.iter().filter(|s| s.kind == "variable").collect();
        assert_eq!(vars.len(), 1, "Expected 1 variable, got: {:?}", syms.iter().map(|s| s.kind.clone()).collect::<Vec<_>>());
        assert_eq!(vars[0].name, "MAX_RETRIES");
    }

    #[test]
    fn ts_adapter_extracts_multiple_symbols() {
        let adapter = TypeScriptAdapter::new();
        let src = r#"
import { Logger } from "./logger";

class UserService {
    add(user) { this.users.push(user); }
}

export function createUser(name) {
    return { name };
}

export const VERSION = "1.0.0";
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let kinds: Vec<_> = syms.iter().map(|s| s.kind.as_str()).collect();
        assert!(kinds.contains(&"import"), "kinds: {:?}", kinds);
        assert!(kinds.contains(&"class"), "kinds: {:?}", kinds);
        assert!(kinds.contains(&"function"), "kinds: {:?}", kinds);
        assert!(syms.len() >= 3, "Expected >= 3 symbols, got: {}", syms.len());
    }

    #[test]
    fn ts_adapter_nested_symbols_with_scope() {
        let adapter = TypeScriptAdapter::new();
        let src = r#"
class Container {
    public method() { return 42; }
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert!(syms.iter().any(|s| s.kind == "class"), "symbols: {:?}", syms.iter().map(|s| &s.kind).collect::<Vec<_>>());
        assert!(syms.iter().any(|s| s.kind == "method"), "symbols: {:?}", syms.iter().map(|s| &s.kind).collect::<Vec<_>>());
    }

    #[test]
    fn ts_adapter_symbol_has_line_range() {
        let adapter = TypeScriptAdapter::new();
        let src = "function testFn() {\n    const x = 1;\n    return x + 1;\n}";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert!(!syms.is_empty());
        assert_eq!(syms[0].start_line, 0);
        assert!(syms[0].end_line >= 3);
    }

    #[test]
    fn ts_adapter_symbol_has_signature() {
        let adapter = TypeScriptAdapter::new();
        let src = "function compute(a, b) { return a + b; }";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert!(syms[0].signature.is_some());
        assert!(syms[0].signature.as_ref().unwrap().contains("compute"));
    }

    #[test]
    fn ts_adapter_box_clone() {
        let adapter = TypeScriptAdapter::new();
        let cloned = adapter.box_clone();
        let src = "function test() {}";
        let parsed = cloned.parse_source(src).expect("clone should work");
        let syms = cloned.extract_symbols(&parsed).unwrap();
        assert_eq!(syms.len(), 1);
    }

    #[test]
    fn ts_adapter_source_only_whitespace() {
        let adapter = TypeScriptAdapter::new();
        let src = "   \n\n  \t  ";
        let parsed = adapter.parse_source(src).expect("should not crash on whitespace");
        assert_eq!(parsed.language, "typescript");
    }

    #[test]
    fn ts_adapter_arrow_functions() {
        let adapter = TypeScriptAdapter::new();
        let src = r#"
const greet = (name) => `Hello, ${name}`;
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let functions: Vec<_> = syms.iter().filter(|s| s.kind == "function").collect();
        assert!(functions.len() >= 1, "Expected functions, got: {:?}", syms.iter().map(|s| &s.kind).collect::<Vec<_>>());
    }

    #[test]
    fn ts_adapter_registers_both_typescript_and_javascript() {
        let registry = crate::app::bootstrap::Registry::new();
        crate::adapters::typescript::register_to(&registry);
        assert!(registry.get("typescript").is_some());
        assert!(registry.get("javascript").is_some());
        let mut langs = registry.list_languages();
        langs.sort();
        assert_eq!(langs, vec!["javascript", "typescript"]);
    }
}
