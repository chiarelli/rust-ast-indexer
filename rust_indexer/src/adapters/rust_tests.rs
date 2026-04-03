#[cfg(test)]
mod tests {
    use crate::adapters::{LanguageAdapter, rust::RustAdapter};

    #[test]
    fn rust_adapter_parses_simple_fn() {
        let adapter = RustAdapter::new();
        let src = "fn hello(name: &str) -> String { format!(\"hi {}\", name) }";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        assert_eq!(parsed.language, "rust");
        assert_eq!(parsed.source_len, src.len());

        let syms = adapter.extract_symbols(&parsed).expect("extract_symbols should run");
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].name, "hello");
        assert_eq!(syms[0].kind, "function");
    }

    #[test]
    fn rust_adapter_handles_empty_source() {
        let adapter = RustAdapter::new();
        let src = "";
        let parsed = adapter.parse_source(src).expect("parse should succeed on empty");
        assert_eq!(parsed.language, "rust");
        assert_eq!(parsed.source_len, 0);
    }

    #[test]
    fn rust_adapter_extracts_struct() {
        let adapter = RustAdapter::new();
        let src = r#"
pub struct User {
    pub name: String,
    pub age: u32,
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let structs: Vec<_> = syms.iter().filter(|s| s.kind == "struct").collect();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].name, "User");
    }

    #[test]
    fn rust_adapter_extracts_enum() {
        let adapter = RustAdapter::new();
        let src = r#"
pub enum Status {
    Active,
    Inactive,
    Pending { reason: String },
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let enums: Vec<_> = syms.iter().filter(|s| s.kind == "enum").collect();
        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].name, "Status");
    }

    #[test]
    fn rust_adapter_extracts_trait() {
        let adapter = RustAdapter::new();
        let src = r#"
pub trait Repository {
    fn find(&self, id: u32) -> Option<String>;
    fn save(&mut self, item: String);
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let traits: Vec<_> = syms.iter().filter(|s| s.kind == "trait").collect();
        assert_eq!(traits.len(), 1);
        assert_eq!(traits[0].name, "Repository");
    }

    #[test]
    fn rust_adapter_extracts_impl() {
        let adapter = RustAdapter::new();
        let src = r#"
impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "User({})", self.name)
    }
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let impls: Vec<_> = syms.iter().filter(|s| s.kind == "impl").collect();
        assert_eq!(impls.len(), 1);
    }

    #[test]
    fn rust_adapter_extracts_mod() {
        let adapter = RustAdapter::new();
        let src = "mod my_module { fn internal() {} }";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let mods: Vec<_> = syms.iter().filter(|s| s.kind == "mod").collect();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].name, "my_module");
    }

    #[test]
    fn rust_adapter_extracts_use_declaration() {
        let adapter = RustAdapter::new();
        let src = "use std::collections::HashMap;";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let uses: Vec<_> = syms.iter().filter(|s| s.kind == "use").collect();
        assert_eq!(uses.len(), 1);
    }

    #[test]
    fn rust_adapter_extracts_import_edges() {
        let adapter = RustAdapter::new();
        let src = "use std::collections::HashMap;";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let edges = adapter.extract_imports(&parsed).expect("extract_imports should run");
        assert_eq!(edges.len(), 1);
        let e = &edges[0];
        assert!(e.from_file.contains("<source>") || e.from_file == "<source>");
        assert!(e.to_module.contains("std::collections") || e.to_module.contains("HashMap") || e.to_module.contains("std"));
        assert_eq!(e.import_kind, "named");
        assert!(!e.resolved);
    }

    #[test]
    fn rust_adapter_extracts_const() {
        let adapter = RustAdapter::new();
        let src = "const MAX_SIZE: usize = 100;";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let consts: Vec<_> = syms.iter().filter(|s| s.kind == "const").collect();
        assert_eq!(consts.len(), 1);
    }

    #[test]
    fn rust_adapter_extracts_static() {
        let adapter = RustAdapter::new();
        let src = "static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let statics: Vec<_> = syms.iter().filter(|s| s.kind == "static").collect();
        assert_eq!(statics.len(), 1);
    }

    #[test]
    fn rust_adapter_multiple_symbols() {
        let adapter = RustAdapter::new();
        let src = r#"
use std::io;
mod helpers;
pub struct Config { pub port: u16 }
pub enum Mode { Debug, Release }
pub fn run() { println!("running"); }
const VERSION: &str = "1.0";
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert!(syms.len() >= 5);
        let kinds: Vec<_> = syms.iter().map(|s| s.kind.as_str()).collect();
        assert!(kinds.contains(&"use"));
        assert!(kinds.contains(&"mod"));
        assert!(kinds.contains(&"struct"));
        assert!(kinds.contains(&"enum"));
        assert!(kinds.contains(&"function"));
        assert!(kinds.contains(&"const"));
    }

    #[test]
    fn rust_adapter_nested_symbols_with_scope() {
        let adapter = RustAdapter::new();
        let src = r#"
mod my_module {
    pub fn inner_fn() {}
    pub struct InnerStruct { field: u32 }
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert!(syms.iter().any(|s| s.kind == "mod"));
        assert!(syms.iter().any(|s| s.kind == "function"));
        assert!(syms.iter().any(|s| s.kind == "struct"));
    }

    #[test]
    fn rust_adapter_symbol_has_line_range() {
        let adapter = RustAdapter::new();
        let src = "fn test_fn() {\n    let x = 1;\n    x + 1\n}";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert_eq!(syms.len(), 1);
        assert_eq!(syms[0].start_line, 0);
        assert!(syms[0].end_line >= 2);
    }

    #[test]
    fn rust_adapter_symbol_has_signature() {
        let adapter = RustAdapter::new();
        let src = "fn compute(a: u32, b: u32) -> u32 { a + b }";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert!(syms[0].signature.is_some());
        assert!(syms[0].signature.as_ref().unwrap().contains("compute"));
    }

    #[test]
    fn rust_adapter_box_clone() {
        let adapter = RustAdapter::new();
        let cloned = adapter.box_clone();
        let src = "fn test() {}";
        let parsed = cloned.parse_source(src).expect("clone should work");
        let syms = cloned.extract_symbols(&parsed).unwrap();
        assert_eq!(syms.len(), 1);
    }

    #[test]
    fn rust_adapter_source_only_whitespace() {
        let adapter = RustAdapter::new();
        let src = "   \n\n  \t  ";
        let parsed = adapter.parse_source(src).expect("should not crash on whitespace");
        assert_eq!(parsed.language, "rust");
    }
}
