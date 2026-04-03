pub use normalize::*;

#[allow(clippy::module_inception)]
mod normalize {
    use crate::domain::types::{ImportEdge, NormalizedSymbol, Symbol};
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

    /// Heuristics for normalizing an import edge based on the raw import text and language.
    /// 
    /// The adapters produce a raw `ImportEdge` with `to_module` containing the full import
    /// text and other fields set to defaults. This function refines those fields:
    /// - `to_module`: extracts just the module path
    /// - `imported_symbol`: extracts the imported symbol name when detectable
    /// - `alias`: extracts rename/alias information
    /// - `import_kind`: refines to `named`, `default`, `namespace`, `side_effect`, or `reexport`
    /// - `resolved`: set to true for local-relative imports (relative paths, `crate::`)
    pub fn normalize_import(edge: &ImportEdge, language: &str) -> ImportEdge {
        let raw = &edge.to_module;

        match language {
            "rust" => normalize_rust_import(edge, raw),
            "typescript" | "javascript" => normalize_ts_import(edge, raw),
            "java" => normalize_java_import(edge, raw),
            "go" => normalize_go_import(edge, raw),
            _ => edge.clone(),
        }
    }

    fn normalize_rust_import(edge: &ImportEdge, raw: &str) -> ImportEdge {
        let trimmed = raw.trim();
        // Detect reexports
        let is_reexport = trimmed.starts_with("pub");
        let import_body = if is_reexport {
            trimmed.strip_prefix("pub ").unwrap_or(trimmed)
        } else {
            trimmed
        };

        // Strip "use " prefix
        let body = import_body.strip_prefix("use ").unwrap_or(import_body).trim().trim_end_matches(';').trim();

        // Detect glob: use foo::*
        if body.ends_with("::*") {
            let module = body.strip_suffix("::*").unwrap_or(body).trim();
            return ImportEdge {
                id: edge.id.clone(),
                from_file: edge.from_file.clone(),
                to_module: module.to_string(),
                imported_symbol: None,
                alias: None,
                import_kind: "namespace".to_string(),
                location: edge.location.clone(),
                resolved: is_local_rust_module(raw),
            };
        }

        // Detect alias: use foo::bar as baz
        if let Some(idx) = body.rfind(" as ") {
            let module_part = body[..idx].trim();
            let alias_part = body[idx + 4..].trim();
            return ImportEdge {
                id: edge.id.clone(),
                from_file: edge.from_file.clone(),
                to_module: module_part.to_string(),
                imported_symbol: module_part.split("::").last().map(String::from),
                alias: Some(alias_part.to_string()),
                import_kind: "named".to_string(),
                location: edge.location.clone(),
                resolved: is_local_rust_module(raw),
            };
        }

        // Simple use - extract module path and imported symbol
        let parts: Vec<&str> = body.split("::").collect();
        let (module, symbol) = if parts.len() > 1 {
            // module is everything except last segment
            (parts[..parts.len() - 1].join("::"), Some(parts[parts.len() - 1].to_string()))
        } else {
            (body.to_string(), None)
        };

        ImportEdge {
            id: edge.id.clone(),
            from_file: edge.from_file.clone(),
            to_module: module,
            imported_symbol: symbol,
            alias: None,
            import_kind: if is_reexport {
                "reexport".to_string()
            } else {
                "named".to_string()
            },
            location: edge.location.clone(),
            resolved: is_local_rust_module(raw),
        }
    }

    fn is_local_rust_module(raw: &str) -> bool {
        raw.contains("crate::") || raw.contains("self::") || raw.contains("super::") || raw.contains("./") || raw.contains("../")
    }

    fn normalize_ts_import(edge: &ImportEdge, raw: &str) -> ImportEdge {
        let trimmed = raw.trim();

        // Detect: import "module" (side effect)
        let bare_string = trimmed
            .strip_prefix("import ")
            .and_then(|s| s.strip_suffix(";"))
            .map(|s| s.trim());
        if let Some(content) = bare_string {
            // Check if it's just a quoted string
            if content.starts_with('"') && content.ends_with('"')
                || content.starts_with('\'') && content.ends_with('\'') {
                return ImportEdge {
                    id: edge.id.clone(),
                    from_file: edge.from_file.clone(),
                    to_module: content[1..content.len() - 1].to_string(),
                    imported_symbol: None,
                    alias: None,
                    import_kind: "side_effect".to_string(),
                    location: edge.location.clone(),
                    resolved: is_local_relative_import(content),
                };
            }

            // Detect: import * as alias from "module"
            if let Some(rest) = content.strip_prefix("* as ") {
                if let Some(from_pos) = rest.find(" from ") {
                    let alias = rest[..from_pos].trim();
                    let module_part = extract_ts_module(&rest[from_pos + 6..]);
                    return ImportEdge {
                        id: edge.id.clone(),
                        from_file: edge.from_file.clone(),
                        to_module: module_part.clone(),
                        imported_symbol: None,
                        alias: Some(alias.to_string()),
                        import_kind: "namespace".to_string(),
                        location: edge.location.clone(),
                        resolved: is_local_relative_import(&module_part),
                    };
                }
            }

            // Detect: import { symbols } from "module" or import symbols from "module"
            if let Some(from_pos) = content.find(" from ") {
                let before_from = content[..from_pos].trim();
                let module_part = extract_ts_module(&content[from_pos + 6..]);

                // Default import: import foo from "module"
                if !before_from.starts_with('{') && !before_from.starts_with('*') {
                    return ImportEdge {
                        id: edge.id.clone(),
                        from_file: edge.from_file.clone(),
                        to_module: module_part.clone(),
                        imported_symbol: Some("default".to_string()),
                        alias: Some(before_from.to_string()),
                        import_kind: "default".to_string(),
                        location: edge.location.clone(),
                        resolved: is_local_relative_import(&module_part),
                    };
                }

                // Named import with braces: import { x, y as z } from "module"
                if let Some(symbols) = before_from.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                    let symbols = symbols.trim();
                    // Check for alias
                    if let Some(idx) = symbols.rfind(" as ") {
                        let symbol = symbols[..idx].trim().to_string();
                        let alias = symbols[idx + 4..].trim().to_string();
                        return ImportEdge {
                            id: edge.id.clone(),
                            from_file: edge.from_file.clone(),
                            to_module: module_part.clone(),
                            imported_symbol: Some(symbol),
                            alias: Some(alias),
                            import_kind: "named".to_string(),
                            location: edge.location.clone(),
                            resolved: is_local_relative_import(&module_part),
                        };
                    }
                    // Single named import
                    return ImportEdge {
                        id: edge.id.clone(),
                        from_file: edge.from_file.clone(),
                        to_module: module_part.clone(),
                        imported_symbol: if symbols.is_empty() { None } else { Some(symbols.to_string()) },
                        alias: None,
                        import_kind: "named".to_string(),
                        location: edge.location.clone(),
                        resolved: is_local_relative_import(&module_part),
                    };
                }
            }
        }

        // Fallback: couldn't parse, keep raw
        edge.clone()
    }

    fn extract_ts_module(s: &str) -> String {
        let s = s.trim().trim_end_matches(';').trim();
        if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
            s[1..s.len() - 1].to_string()
        } else {
            s.to_string()
        }
    }

    fn is_local_relative_import(text: &str) -> bool {
        text.starts_with("./") || text.starts_with("../")
    }

    fn normalize_java_import(edge: &ImportEdge, _raw: &str) -> ImportEdge {
        let trimmed = edge.to_module.trim();
        let body = trimmed.strip_prefix("import ").unwrap_or(trimmed).trim().trim_end_matches(';').trim();

        // Detect static imports
        if let Some(rest) = body.strip_prefix("static ") {
            let last_dot = rest.rfind('.');
            let (module, symbol) = if let Some(pos) = last_dot {
                (rest[..pos].to_string(), Some(rest[pos + 1..].to_string()))
            } else {
                (rest.to_string(), None)
            };
            return ImportEdge {
                id: edge.id.clone(),
                from_file: edge.from_file.clone(),
                to_module: module,
                imported_symbol: symbol,
                alias: None,
                import_kind: "named".to_string(),
                location: edge.location.clone(),
                resolved: false,
            };
        }

        // Normal java import: package.Class
        let last_dot = body.rfind('.');
        let (module, symbol) = if let Some(pos) = last_dot {
            (body[..pos].to_string(), Some(body[pos + 1..].to_string()))
        } else {
            (body.to_string(), None)
        };

        ImportEdge {
            id: edge.id.clone(),
            from_file: edge.from_file.clone(),
            to_module: module,
            imported_symbol: symbol,
            alias: None,
            import_kind: "named".to_string(),
            location: edge.location.clone(),
            resolved: false,
        }
    }

    fn normalize_go_import(edge: &ImportEdge, _raw: &str) -> ImportEdge {
        let trimmed = edge.to_module.trim();
        let body = trimmed.strip_prefix("import ").unwrap_or(trimmed).trim().trim_end_matches(';').trim();

        // Strip quotes
        let module_str = if (body.starts_with('"') && body.ends_with('"'))
            || (body.starts_with('`') && body.ends_with('`')) {
            &body[1..body.len() - 1]
        } else {
            body
        };

        // Go imports with alias: import alias "path"
        // The adapter stores full text "import alias \"path\"" or "import \"path\""
        // Simple heuristic: if there are two quoted segments, first is alias
        ImportEdge {
            id: edge.id.clone(),
            from_file: edge.from_file.clone(),
            to_module: module_str.to_string(),
            imported_symbol: None,
            alias: None,
            import_kind: "named".to_string(),
            location: edge.location.clone(),
            resolved: module_str.starts_with("./") || module_str.starts_with("../"),
        }
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

        mod normalize_import {
            use super::*;

            #[test]
            fn rust_use_single() {
                let edge = make_import_edge("use std::collections::HashMap;");
                let norm = normalize_import(&edge, "rust");
                assert_eq!(norm.to_module, "std::collections");
                assert_eq!(norm.imported_symbol, Some("HashMap".to_string()));
                assert_eq!(norm.import_kind, "named");
                assert!(!norm.resolved);
                assert!(norm.alias.is_none());
            }

            #[test]
            fn rust_use_with_alias() {
                let edge = make_import_edge("use std::collections::HashMap as Map;");
                let norm = normalize_import(&edge, "rust");
                assert_eq!(norm.to_module, "std::collections::HashMap");
                assert_eq!(norm.imported_symbol, Some("HashMap".to_string()));
                assert_eq!(norm.alias, Some("Map".to_string()));
                assert_eq!(norm.import_kind, "named");
                assert!(!norm.resolved);
            }

            #[test]
            fn rust_use_glob() {
                let edge = make_import_edge("use std::io::*;");
                let norm = normalize_import(&edge, "rust");
                assert_eq!(norm.to_module, "std::io");
                assert_eq!(norm.imported_symbol, None);
                assert_eq!(norm.import_kind, "namespace");
                assert!(!norm.resolved);
            }

            #[test]
            fn rust_pub_use_is_reexport() {
                let edge = make_import_edge("pub use internal::helper;");
                let norm = normalize_import(&edge, "rust");
                assert_eq!(norm.to_module, "internal");
                assert_eq!(norm.import_kind, "reexport");
            }

            #[test]
            fn rust_crate_path_is_resolved() {
                let edge = make_import_edge("use crate::utils::format_name;");
                let norm = normalize_import(&edge, "rust");
                assert_eq!(norm.to_module, "crate::utils");
                assert_eq!(norm.imported_symbol, Some("format_name".to_string()));
                assert!(norm.resolved);
            }

            #[test]
            fn rust_self_path_is_resolved() {
                let edge = make_import_edge("use self::helpers;");
                let norm = normalize_import(&edge, "rust");
                assert!(norm.resolved);
            }

            #[test]
            fn ts_import_named() {
                let edge = make_import_edge("import { useState } from \"react\";");
                let norm = normalize_import(&edge, "typescript");
                assert_eq!(norm.to_module, "react");
                assert_eq!(norm.imported_symbol, Some("useState".to_string()));
                assert_eq!(norm.import_kind, "named");
                assert!(!norm.resolved);
            }

            #[test]
            fn ts_import_named_with_alias() {
                let edge = make_import_edge("import { default as React } from \"react\";");
                let norm = normalize_import(&edge, "typescript");
                assert_eq!(norm.to_module, "react");
                assert_eq!(norm.imported_symbol, Some("default".to_string()));
                assert_eq!(norm.alias, Some("React".to_string()));
                assert_eq!(norm.import_kind, "named");
            }

            #[test]
            fn ts_import_default() {
                let edge = make_import_edge("import _ from \"lodash\";");
                let norm = normalize_import(&edge, "typescript");
                assert_eq!(norm.to_module, "lodash");
                assert_eq!(norm.imported_symbol, Some("default".to_string()));
                assert_eq!(norm.alias, Some("_".to_string()));
                assert_eq!(norm.import_kind, "default");
            }

            #[test]
            fn ts_import_namespace() {
                let edge = make_import_edge("import * as fs from \"fs\";");
                let norm = normalize_import(&edge, "typescript");
                assert_eq!(norm.to_module, "fs");
                assert_eq!(norm.alias, Some("fs".to_string()));
                assert_eq!(norm.import_kind, "namespace");
                assert!(norm.imported_symbol.is_none());
            }

            #[test]
            fn ts_import_side_effect() {
                let edge = make_import_edge("import \"core-js/promise\";");
                let norm = normalize_import(&edge, "typescript");
                assert_eq!(norm.to_module, "core-js/promise");
                assert_eq!(norm.import_kind, "side_effect");
                assert!(norm.imported_symbol.is_none());
                assert!(norm.alias.is_none());
            }

            #[test]
            fn ts_relative_import_resolved() {
                let edge = make_import_edge("import { foo } from \"./utils\";");
                let norm = normalize_import(&edge, "typescript");
                assert!(norm.resolved);
            }

            #[test]
            fn java_import_class() {
                let edge = make_import_edge("import java.util.List;");
                let norm = normalize_import(&edge, "java");
                assert_eq!(norm.to_module, "java.util");
                assert_eq!(norm.imported_symbol, Some("List".to_string()));
                assert_eq!(norm.import_kind, "named");
                assert!(!norm.resolved);
            }

            #[test]
            fn java_static_import() {
                let edge = make_import_edge("import static java.util.Collections.emptyList;");
                let norm = normalize_import(&edge, "java");
                assert_eq!(norm.to_module, "java.util.Collections");
                assert_eq!(norm.imported_symbol, Some("emptyList".to_string()));
            }

            #[test]
            fn go_import() {
                let edge = make_import_edge("import \"fmt\"");
                let norm = normalize_import(&edge, "go");
                assert_eq!(norm.to_module, "fmt");
                assert_eq!(norm.import_kind, "named");
                assert!(!norm.resolved);
            }

            #[test]
            fn go_relative_import_resolved() {
                let edge = make_import_edge("import \"./utils\"");
                let norm = normalize_import(&edge, "go");
                assert!(norm.resolved);
            }

            #[test]
            fn unknown_language_passes_through() {
                let edge = make_import_edge("use something;");
                let norm = normalize_import(&edge, "ocaml");
                assert!(norm.to_module.contains("something"));
            }

            fn make_import_edge(raw_text: &str) -> ImportEdge {
                ImportEdge {
                    id: "ie_test".to_string(),
                    from_file: "src/lib.rs".to_string(),
                    to_module: raw_text.to_string(),
                    imported_symbol: None,
                    alias: None,
                    import_kind: "named".to_string(),
                    location: crate::domain::types::Location { start_line: 1, start_col: 0, end_line: 1, end_col: 30 },
                    resolved: false,
                }
            }
        }
    }
}
