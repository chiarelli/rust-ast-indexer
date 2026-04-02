pub use normalize::*;

mod normalize {
    use crate::domain::types::{NormalizedSymbol, Symbol};
    use std::collections::HashMap;

    /// Errors that can occur during symbol normalization
    #[derive(Debug)]
    pub enum NormalizeError {
        EmptySymbolName,
        EmptySymbolId,
    }

    impl std::fmt::Display for NormalizeError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                NormalizeError::EmptySymbolName => write!(f, "symbol name must not be empty"),
                NormalizeError::EmptySymbolId => write!(f, "symbol id must not be empty"),
            }
        }
    }

    impl std::error::Error for NormalizeError {}

    pub type Result<T> = std::result::Result<T, NormalizeError>;

    /// Normalize a single Symbol into NormalizedSymbol
    fn normalize_symbol(symbol: &Symbol, language: Option<&str>) -> Result<NormalizedSymbol> {
        if symbol.name.is_empty() {
            return Err(NormalizeError::EmptySymbolName);
        }
        if symbol.id.is_empty() {
            return Err(NormalizeError::EmptySymbolId);
        }

        let qualified_name = build_qualified_name(&symbol.name, symbol.scope.as_deref());

        Ok(NormalizedSymbol {
            id: symbol.id.clone(),
            name: symbol.name.clone(),
            kind: symbol.kind.clone(),
            qualified_name,
            file_path: symbol.file_path.clone(),
            start_line: symbol.start_line,
            end_line: symbol.end_line,
            signature: symbol.signature.clone(),
            language: language.map(String::from),
            is_overloaded: false,
            overload_index: 0,
        })
    }

    /// Build a fully qualified name combining scope and symbol name
    fn build_qualified_name(name: &str, scope: Option<&str>) -> String {
        match scope {
            Some("") | None => name.to_string(),
            Some(s) => format!("{}::{name}", s.trim().trim_end_matches("::")),
        }
    }

    /// Normalize multiple symbols, detecting overloads within the same file+kind+name
    pub fn normalize_symbols(symbols: &[Symbol], file_language: Option<&str>) -> Vec<NormalizedSymbol> {
        // Group by (file_path, kind, name) to detect overloads
        let mut groups: HashMap<(String, String, String), Vec<NormalizedSymbol>> = HashMap::new();

        for symbol in symbols {
            if let Ok(normalized) = normalize_symbol(symbol, file_language) {
                let key = (
                    normalized.file_path.clone(),
                    normalized.kind.clone(),
                    normalized.name.clone(),
                );
                groups.entry(key).or_default().push(normalized);
            }
        }

        // Mark overloads and assign indices
        let mut result = Vec::new();
        for (_, mut group) in groups {
            if group.len() > 1 {
                group.sort_by_key(|s| (s.start_line, s.end_line));
                for (idx, s) in group.iter_mut().enumerate() {
                    s.is_overloaded = true;
                    s.overload_index = idx;
                }
            }
            result.extend(group);
        }

        // Stable sort by file path and starting line
        result.sort_by(|a, b| {
            a.file_path
                .cmp(&b.file_path)
                .then(a.start_line.cmp(&b.start_line))
        });

        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Helper to build a Symbol for tests
        fn make_symbol(name: &str, kind: &str, scope: Option<&str>, file_path: &str, start_line: usize, end_line: usize) -> Symbol {
            Symbol {
                id: format!("{}:{}", file_path, name),
                name: name.to_string(),
                kind: kind.to_string(),
                scope: scope.map(String::from),
                file_path: file_path.to_string(),
                start_line,
                end_line,
                signature: None,
            }
        }

        mod build_qualified_name {
            use super::*;

            #[test]
            fn returns_name_when_no_scope() {
                let qname = build_qualified_name("MyFunction", None);
                assert_eq!(qname, "MyFunction");
            }

            #[test]
            fn returns_name_when_scope_empty() {
                let qname = build_qualified_name("MyFunction", Some(""));
                assert_eq!(qname, "MyFunction");
            }

            #[test]
            fn combines_scope_and_name() {
                let qname = build_qualified_name("getUsers", Some("UserService"));
                assert_eq!(qname, "UserService::getUsers");
            }

            #[test]
            fn handles_multiple_nesting_levels() {
                let qname = build_qualified_name("query", Some("db::Repository"));
                assert_eq!(qname, "db::Repository::query");
            }

            #[test]
            fn trims_trailing_scope_colons() {
                let qname = build_qualified_name("fn", Some("mod::"));
                assert_eq!(qname, "mod::fn");
            }
        }

        mod normalize_symbol {
            use super::*;

            #[test]
            fn normalizes_simple_function() {
                let sym = make_symbol("hello", "function", None, "main.rs", 0, 5);
                let norm = normalize_symbol(&sym, Some("rust")).expect("should succeed");
                assert_eq!(norm.name, "hello");
                assert_eq!(norm.kind, "function");
                assert_eq!(norm.qualified_name, "hello");
                assert_eq!(norm.language.as_deref(), Some("rust"));
                assert!(!norm.is_overloaded);
                assert_eq!(norm.overload_index, 0);
            }

            #[test]
            fn normalizes_nested_class_method() {
                let sym = make_symbol("addUser", "method", Some("UserService"), "UserService.ts", 10, 15);
                let norm = normalize_symbol(&sym, Some("typescript")).expect("should succeed");
                assert_eq!(norm.qualified_name, "UserService::addUser");
            }

            #[test]
            fn normalizes_deeply_nested_symbol() {
                let sym = make_symbol("handle", "function", Some("api::routes::users"), "routes.rs", 20, 30);
                let norm = normalize_symbol(&sym, Some("rust")).expect("should succeed");
                assert_eq!(norm.qualified_name, "api::routes::users::handle");
            }

            #[test]
            fn empty_name_returns_error() {
                let sym = make_symbol("", "function", None, "main.rs", 0, 5);
                assert!(matches!(normalize_symbol(&sym, None), Err(NormalizeError::EmptySymbolName)));
            }

            #[test]
            fn empty_id_returns_error() {
                let mut sym = make_symbol("fn", "function", None, "main.rs", 0, 5);
                sym.id = String::new();
                assert!(matches!(normalize_symbol(&sym, None), Err(NormalizeError::EmptySymbolId)));
            }

            #[test]
            fn normalizes_without_language() {
                let sym = make_symbol("main", "function", None, "main.rs", 0, 10);
                let norm = normalize_symbol(&sym, None).expect("should succeed");
                assert_eq!(norm.language, None);
            }

            #[test]
            fn normalizes_symbol_with_signature() {
                let mut sym = make_symbol("compute", "function", None, "math.rs", 0, 3);
                sym.signature = Some("fn compute(a: u32, b: u32) -> u32".to_string());
                let norm = normalize_symbol(&sym, Some("rust")).expect("should succeed");
                assert_eq!(norm.signature.as_deref(), Some("fn compute(a: u32, b: u32) -> u32"));
            }
        }

        mod normalize_symbols {
            use super::*;

            #[test]
            fn normalizes_multiple_symbols() {
                let symbols = vec![
                    make_symbol("main", "function", None, "main.rs", 0, 10),
                    make_symbol("Helper", "struct", None, "main.rs", 12, 20),
                    make_symbol("helper_fn", "function", Some("Helper"), "main.rs", 13, 18),
                ];
                let normalized = normalize_symbols(&symbols, Some("rust"));
                assert_eq!(normalized.len(), 3);
                assert_eq!(normalized[0].name, "main");
                assert_eq!(normalized[2].qualified_name, "Helper::helper_fn");
            }

            #[test]
            fn detects_overloaded_functions() {
                let symbols = vec![
                    make_symbol("process", "function", None, "handler.rs", 0, 5),
                    make_symbol("process", "function", None, "handler.rs", 7, 12),
                ];
                let normalized = normalize_symbols(&symbols, Some("rust"));
                assert_eq!(normalized.len(), 2);
                assert!(normalized[0].is_overloaded);
                assert!(normalized[1].is_overloaded);
                assert_eq!(normalized[0].overload_index, 0);
                assert_eq!(normalized[1].overload_index, 1);
            }

            #[test]
            fn overloads_sorted_by_position() {
                let symbols = vec![
                    make_symbol("process", "function", None, "x.rs", 10, 15),
                    make_symbol("process", "function", None, "x.rs", 0, 5),
                ];
                let normalized = normalize_symbols(&symbols, Some("rust"));
                assert_eq!(normalized[0].start_line, 0);
                assert_eq!(normalized[0].overload_index, 0);
                assert_eq!(normalized[1].start_line, 10);
                assert_eq!(normalized[1].overload_index, 1);
            }

            #[test]
            fn no_overloads_when_single_symbol() {
                let symbols = vec![make_symbol("unique_fn", "function", None, "a.rs", 0, 5)];
                let normalized = normalize_symbols(&symbols, Some("rust"));
                assert_eq!(normalized.len(), 1);
                assert!(!normalized[0].is_overloaded);
                assert_eq!(normalized[0].overload_index, 0);
            }

            #[test]
            fn no_overloads_across_different_files() {
                let symbols = vec![
                    make_symbol("init", "function", None, "file_a.rs", 0, 5),
                    make_symbol("init", "function", None, "file_b.rs", 0, 3),
                ];
                let normalized = normalize_symbols(&symbols, Some("rust"));
                assert_eq!(normalized.len(), 2);
                assert!(!normalized.iter().any(|s| s.is_overloaded));
            }

            #[test]
            fn no_overloads_across_different_kinds() {
                let symbols = vec![
                    make_symbol("Config", "function", None, "a.rs", 0, 5),
                    make_symbol("Config", "struct", None, "a.rs", 7, 15),
                ];
                let normalized = normalize_symbols(&symbols, Some("rust"));
                assert!(!normalized.iter().any(|s| s.is_overloaded));
            }

            #[test]
            fn skips_symbols_with_empty_name() {
                let symbols = vec![
                    make_symbol("valid", "function", None, "a.rs", 0, 5),
                    make_symbol("", "function", None, "a.rs", 7, 10),
                ];
                let normalized = normalize_symbols(&symbols, Some("rust"));
                assert_eq!(normalized.len(), 1);
                assert_eq!(normalized[0].name, "valid");
            }

            #[test]
            fn skips_symbols_with_empty_id() {
                let mut bad = make_symbol("bad", "function", None, "a.rs", 0, 5);
                bad.id = String::new();
                let symbols = vec![make_symbol("good", "function", None, "a.rs", 7, 10), bad];
                let normalized = normalize_symbols(&symbols, Some("rust"));
                assert_eq!(normalized.len(), 1);
                assert_eq!(normalized[0].name, "good");
            }

            #[test]
            fn result_sorted_by_file_and_line() {
                let symbols = vec![
                    make_symbol("zzz", "function", None, "z.rs", 0, 5),
                    make_symbol("aaa", "function", None, "a.rs", 10, 15),
                    make_symbol("bbb", "struct", None, "a.rs", 0, 5),
                ];
                let normalized = normalize_symbols(&symbols, None);
                assert_eq!(normalized[0].file_path, "a.rs");
                assert_eq!(normalized[0].start_line, 0);
                assert_eq!(normalized[1].file_path, "a.rs");
                assert_eq!(normalized[1].start_line, 10);
                assert_eq!(normalized[2].file_path, "z.rs");
            }

            #[test]
            fn handles_empty_input() {
                let normalized = normalize_symbols(&[], Some("rust"));
                assert!(normalized.is_empty());
            }

            #[test]
            fn handles_nested_scopes_correctly() {
                let symbols = vec![
                    make_symbol("Module", "mod", None, "lib.rs", 0, 10),
                    make_symbol("Nested", "struct", Some("Module"), "lib.rs", 1, 5),
                    make_symbol("deep_fn", "function", Some("Module::Nested"), "lib.rs", 2, 4),
                ];
                let normalized = normalize_symbols(&symbols, Some("rust"));
                let deep = normalized.iter().find(|s| s.name == "deep_fn").expect("should find deep_fn");
                assert_eq!(deep.qualified_name, "Module::Nested::deep_fn");
            }
        }
    }
}
