#[cfg(feature = "parsing")]
mod go_adapter {
    use crate::adapters::LanguageAdapter;
    use crate::domain::parser::ParsedFile;
    use crate::domain::types::{CallEdge, ImportEdge, Location, Symbol};
    use anyhow::Result;
    use tree_sitter::{Parser, Tree};

    fn go_language() -> tree_sitter::Language {
        tree_sitter_go::language()
    }

    pub struct GoAdapter;

    impl Default for GoAdapter {
        fn default() -> Self { Self }
    }

    impl GoAdapter {
        pub fn new() -> Self { GoAdapter }

        fn parse_tree(&self, source: &str) -> Result<(Tree, String)> {
            let mut parser = Parser::new();
            parser
                .set_language(go_language())
                .map_err(|e| anyhow::anyhow!("set_language failed: {:?}", e))?;
            let tree = parser
                .parse(source, None)
                .ok_or_else(|| anyhow::anyhow!("parse failed"))?;
            Ok((tree, source.to_string()))
        }

        fn node_type(kind: &str) -> Option<&str> {
            match kind {
                "function_declaration" => Some("function"),
                "method_declaration" => Some("method"),
                "type_declaration" => Some("type"),
                "import_declaration" => Some("import"),
                "var_declaration" => Some("var"),
                "const_declaration" => Some("const"),
                _ => None,
            }
        }

        fn extract_name(node: &tree_sitter::Node, source: &str) -> String {
            // Go: name is often in a field or first identifier
            match node.kind() {
                "var_declaration" | "const_declaration" => {
                    // For var/const, look for identifiers
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "identifier" {
                                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                                    return text.to_string();
                                }
                            }
                        }
                    }
                    "variable".to_string()
                }
                _ => {
                    // Standard: try "name" field
                    if let Some(name_node) = node.child_by_field_name("name") {
                        if let Ok(text) = name_node.utf8_text(source.as_bytes()) {
                            return text.to_string();
                        }
                    }
                    // Fallback: first identifier child
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "identifier" {
                                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                                    return text.to_string();
                                }
                            }
                        }
                    }
                    node.kind().to_string()
                }
            }
        }

        fn walk_tree(
            cursor: &mut tree_sitter::TreeCursor,
            source: &str,
            file_path: &str,
            scope: Option<&str>,
            symbols: &mut Vec<Symbol>,
        ) {
            loop {
                let node = cursor.node();
                let kind = node.kind();

                // Skip ERROR nodes
                if kind != "ERROR" {
                    if let Some(symbol_kind) = Self::node_type(kind) {
                        let name = Self::extract_name(&node, source);
                        let start_line = node.start_position().row;
                        let end_line = node.end_position().row;
                        let signature =
                            node.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
                        let id = format!("{}:{}", file_path, name);

                        symbols.push(Symbol {
                            id,
                            name: name.clone(),
                            kind: symbol_kind.to_string(),
                            scope: scope.map(String::from),
                            file_path: file_path.to_string(),
                            start_line,
                            end_line,
                            signature,
                        });

                        // Update scope when descending into functions/methods
                        let new_scope = name.clone();
                        if cursor.goto_first_child() {
                            Self::walk_tree(
                                cursor,
                                source,
                                file_path,
                                Some(&new_scope),
                                symbols,
                            );
                            cursor.goto_parent();
                        }
                    } else if node.child_count() > 0 && cursor.goto_first_child() {
                        Self::walk_tree(
                            cursor,
                            source,
                            file_path,
                            scope,
                            symbols,
                        );
                        cursor.goto_parent();
                    }
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // Collect import edges by traversing the tree and matching "import_declaration" nodes
        fn collect_imports(
            cursor: &mut tree_sitter::TreeCursor,
            source: &str,
            file_path: &str,
            edges: &mut Vec<ImportEdge>,
        ) {
            loop {
                let node = cursor.node();
                let kind = node.kind();

                if kind == "import_declaration" {
                    let start = node.start_position();
                    let end = node.end_position();
                    let text = node.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()).unwrap_or_default();
                    let edge = ImportEdge {
                        id: format!("ie:{}:{}:{}", file_path, start.row, start.column),
                        from_file: file_path.to_string(),
                        to_module: text.trim().to_string(),
                        imported_symbol: None,
                        alias: None,
                        import_kind: "named".to_string(),
                        location: Location { start_line: start.row, start_col: start.column, end_line: end.row, end_col: end.column },
                        resolved: false,
                    };
                    edges.push(edge);
                }

                if node.child_count() > 0 && cursor.goto_first_child() {
                    Self::collect_imports(cursor, source, file_path, edges);
                    cursor.goto_parent();
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // Collect call edges by traversing the tree and matching "call_expression" nodes
        fn collect_calls(
            cursor: &mut tree_sitter::TreeCursor,
            source: &str,
            file_path: &str,
            edges: &mut Vec<CallEdge>,
        ) {
            loop {
                let node = cursor.node();
                let kind = node.kind();

                if kind == "call_expression" {
                    let start = node.start_position();
                    let end = node.end_position();
                    // For call expressions, the function being called can be complex (selector, etc.)
                    // We'll extract the function name from the function position
                    let callee = if let Some(func_node) = node.child(0) {
                        // Try to get the function name text
                        func_node.utf8_text(source.as_bytes()).unwrap_or_default().to_string()
                    } else {
                        // Fallback to full node text
                        node.utf8_text(source.as_bytes()).unwrap_or_default().to_string()
                    };

                    // Determine caller by searching ancestors for a function-like node
                    let mut caller_id = None;
                    let mut ancestor = node.parent();
                    while let Some(a) = ancestor {
                        if Self::node_type(a.kind()).is_some() {
                            let name = Self::extract_name(&a, source);
                            caller_id = Some(format!("{}:{}", file_path, name));
                            break;
                        }
                        ancestor = a.parent();
                    }

                    // Simple heuristic for call kind - in Go we can check for complex expressions
                    let call_kind = if callee.contains('[') || callee.contains('&') || callee.contains('*') {
                        "dynamic"
                    } else {
                        "static"
                    };

                    let edge = CallEdge {
                        id: format!("ce:{}:{}:{}", file_path, start.row, start.column),
                        caller_symbol_id: caller_id,
                        callee_name: callee,
                        callee_symbol_id: None,
                        call_kind: call_kind.to_string(),
                        location: Location { start_line: start.row, start_col: start.column, end_line: end.row, end_col: end.column },
                        resolved: false,
                    };
                    edges.push(edge);
                }

                if node.child_count() > 0 && cursor.goto_first_child() {
                    Self::collect_calls(cursor, source, file_path, edges);
                    cursor.goto_parent();
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        pub fn extract_imports(&self, parsed: &ParsedFile) -> Result<Vec<ImportEdge>> {
            let (tree, _) = self.parse_tree(&parsed.source)?;
            let mut cursor = tree.walk();
            let mut edges = Vec::new();
            Self::collect_imports(&mut cursor, &parsed.source, "<source>", &mut edges);
            Ok(edges)
        }

        pub fn extract_calls(&self, parsed: &ParsedFile) -> Result<Vec<CallEdge>> {
            let (tree, _) = self.parse_tree(&parsed.source)?;
            let mut cursor = tree.walk();
            let mut edges = Vec::new();
            Self::collect_calls(&mut cursor, &parsed.source, "<source>", &mut edges);
            Ok(edges)
        }
    }

    impl LanguageAdapter for GoAdapter {
        fn parse_source(&self, source: &str) -> Result<ParsedFile> {
            let (_, source_str) = self.parse_tree(source)?;
            Ok(ParsedFile {
                language: "go".to_string(),
                source_len: source_str.len(),
                source: source_str,
            })
        }

        fn extract_symbols(&self, parsed: &ParsedFile) -> Result<Vec<Symbol>> {
            let (tree, _) = self.parse_tree(&parsed.source)?;
            let mut cursor = tree.walk();
            let mut symbols = Vec::new();
            Self::walk_tree(&mut cursor, &parsed.source, "<source>", None, &mut symbols);
            Ok(symbols)
        }

        fn box_clone(&self) -> Box<dyn LanguageAdapter> {
            Box::new(GoAdapter::new())
        }
    }

    pub fn register_to(registry: &crate::app::bootstrap::Registry) {
        crate::register_language_adapter!(registry, "go", GoAdapter::new());
    }
}

#[cfg(not(feature = "parsing"))]
mod go_adapter {
    // stub implementation when parsing feature not enabled
}

pub use go_adapter::*;