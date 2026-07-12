#[cfg(feature = "parsing")]
mod rust_adapter {
    use crate::adapters::LanguageAdapter;
    use crate::domain::parser::ParsedFile;
    use crate::domain::types::Symbol;
    use anyhow::Result;
    use tree_sitter::{Parser, Tree};

    fn rust_language() -> tree_sitter::Language {
        tree_sitter_rust::language()
    }

    pub struct RustAdapter;

    impl Default for RustAdapter {
        fn default() -> Self {
            Self
        }
    }

    impl RustAdapter {
        pub fn new() -> Self {
            RustAdapter
        }

        fn parse_tree(&self, source: &str) -> Result<(Tree, String)> {
            let mut parser = Parser::new();
            parser
                .set_language(rust_language())
                .map_err(|e| anyhow::anyhow!("set_language failed: {:?}", e))?;
            let tree = parser
                .parse(source, None)
                .ok_or_else(|| anyhow::anyhow!("parse failed"))?;
            Ok((tree, source.to_string()))
        }

        fn node_type(kind: &str) -> Option<&str> {
            match kind {
                "function_item" => Some("function"),
                "struct_item" => Some("struct"),
                "enum_item" => Some("enum"),
                "impl_item" => Some("impl"),
                "trait_item" => Some("trait"),
                "mod_item" => Some("mod"),
                "use_declaration" => Some("use"),
                "const_item" => Some("const"),
                "static_item" => Some("static"),
                _ => None,
            }
        }

        fn extract_name(node: &tree_sitter::Node, source: &str) -> String {
            node.child_by_field_name("name")
                .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                .unwrap_or("unknown")
                .to_string()
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

                if let Some(symbol_kind) = Self::node_type(kind) {
                    let name = Self::extract_name(&node, source);
                    let start_line = node.start_position().row;
                    let end_line = node.end_position().row;
                    let signature = node
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(|s| s.to_string());
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
                }

                let has_children = node.child_count() > 0;
                let should_descend = has_children;
                if should_descend && cursor.goto_first_child() {
                    Self::walk_tree(cursor, source, file_path, scope, symbols);
                    cursor.goto_parent();
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // Collect import edges by traversing the tree and matching "use_declaration" nodes
        fn collect_imports(
            cursor: &mut tree_sitter::TreeCursor,
            source: &str,
            file_path: &str,
            edges: &mut Vec<crate::domain::types::ImportEdge>,
        ) {
            loop {
                let node = cursor.node();
                let kind = node.kind();

                if kind == "use_declaration" {
                    let start = node.start_position();
                    let end = node.end_position();
                    let text = node
                        .utf8_text(source.as_bytes())
                        .ok()
                        .map(|s| s.to_string())
                        .unwrap_or_default();
                    let edge = crate::domain::types::ImportEdge {
                        id: format!("ie:{}:{}:{}", file_path, start.row, start.column),
                        from_file: file_path.to_string(),
                        to_module: text.trim().to_string(),
                        imported_symbol: None,
                        alias: None,
                        import_kind: "named".to_string(),
                        location: crate::domain::types::Location {
                            start_line: start.row,
                            start_col: start.column,
                            end_line: end.row,
                            end_col: end.column,
                        },
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

        pub fn extract_imports(
            &self,
            parsed: &crate::domain::parser::ParsedFile,
        ) -> Result<Vec<crate::domain::types::ImportEdge>> {
            let (tree, _) = self.parse_tree(&parsed.source)?;
            let mut cursor = tree.walk();
            let mut edges = Vec::new();
            Self::collect_imports(&mut cursor, &parsed.source, "<source>", &mut edges);
            Ok(edges)
        }

        // Collect call edges by traversing the tree and matching "call_expression" and "macro_invocation" nodes
        fn collect_calls(
            cursor: &mut tree_sitter::TreeCursor,
            source: &str,
            file_path: &str,
            edges: &mut Vec<crate::domain::types::CallEdge>,
        ) {
            loop {
                let node = cursor.node();
                let kind = node.kind();

                if kind == "call_expression" || kind == "macro_invocation" {
                    let start = node.start_position();
                    let end = node.end_position();
                    // try to get callee as first child text, fallback to full node text
                    let callee = node
                        .child(0)
                        .and_then(|c| c.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| {
                            node.utf8_text(source.as_bytes())
                                .unwrap_or_default()
                                .to_string()
                        });

                    // determine caller by searching ancestors for a function-like node
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

                    // simple heuristic for call kind
                    let call_kind = if callee.contains('[') || callee.contains('{') {
                        "dynamic"
                    } else {
                        "static"
                    };

                    let edge = crate::domain::types::CallEdge {
                        id: format!("ce:{}:{}:{}", file_path, start.row, start.column),
                        caller_symbol_id: caller_id,
                        callee_name: callee.to_string(),
                        callee_symbol_id: None,
                        call_kind: call_kind.to_string(),
                        location: crate::domain::types::Location {
                            start_line: start.row,
                            start_col: start.column,
                            end_line: end.row,
                            end_col: end.column,
                        },
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

        pub fn extract_calls(
            &self,
            parsed: &crate::domain::parser::ParsedFile,
        ) -> Result<Vec<crate::domain::types::CallEdge>> {
            let (tree, _) = self.parse_tree(&parsed.source)?;
            let mut cursor = tree.walk();
            let mut edges = Vec::new();
            Self::collect_calls(&mut cursor, &parsed.source, "<source>", &mut edges);
            Ok(edges)
        }
    }

    impl LanguageAdapter for RustAdapter {
        fn parse_source(&self, source: &str) -> Result<ParsedFile> {
            let (_, source_str) = self.parse_tree(source)?;
            Ok(ParsedFile {
                language: "rust".to_string(),
                source_len: source.len(),
                source: source.to_string(),
                path: String::new(),
            })
        }

        fn extract_symbols(&self, parsed: &ParsedFile) -> Result<Vec<Symbol>> {
            let (tree, _) = self.parse_tree(&parsed.source)?;
            let mut cursor = tree.walk();
            let mut symbols = Vec::new();
            Self::walk_tree(&mut cursor, &parsed.source, "<source>", None, &mut symbols);
            Ok(symbols)
        }

        fn extract_imports(
            &self,
            parsed: &ParsedFile,
        ) -> Result<Vec<crate::domain::types::ImportEdge>> {
            RustAdapter::extract_imports(self, parsed)
        }

        fn extract_calls(
            &self,
            parsed: &ParsedFile,
        ) -> Result<Vec<crate::domain::types::CallEdge>> {
            RustAdapter::extract_calls(self, parsed)
        }

        fn box_clone(&self) -> Box<dyn LanguageAdapter> {
            Box::new(RustAdapter::new())
        }
    }

    // register at init when feature enabled
    pub fn register_to(registry: &crate::app::bootstrap::Registry) {
        crate::register_language_adapter!(registry, "rust", RustAdapter::new());
    }
}

#[cfg(not(feature = "parsing"))]
mod rust_adapter {
    // stub implementation when parsing feature not enabled
}

#[cfg(feature = "parsing")]
pub use rust_adapter::*;
