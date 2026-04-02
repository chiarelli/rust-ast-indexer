#[cfg(feature = "parsing")]
mod typescript_adapter {
    use crate::adapters::LanguageAdapter;
    use crate::domain::parser::ParsedFile;
    use crate::domain::types::Symbol;
    use anyhow::Result;
    use tree_sitter::{Parser, Tree};

    fn ts_language() -> tree_sitter::Language {
        tree_sitter_javascript::language()
    }

    pub struct TypeScriptAdapter;

    impl Default for TypeScriptAdapter {
        fn default() -> Self { Self }
    }

    impl TypeScriptAdapter {
        pub fn new() -> Self { TypeScriptAdapter }

        fn parse_tree(&self, source: &str) -> Result<(Tree, String)> {
            let mut parser = Parser::new();
            parser
                .set_language(ts_language())
                .map_err(|e| anyhow::anyhow!("set_language failed: {:?}", e))?;
            let tree = parser
                .parse(source, None)
                .ok_or_else(|| anyhow::anyhow!("parse failed"))?;
            Ok((tree, source.to_string()))
        }

        fn node_type(kind: &str) -> Option<&str> {
            match kind {
                "function_declaration" | "arrow_function" | "function_expression" => Some("function"),
                "class_declaration" => Some("class"),
                "enum_declaration" => Some("enum"),
                "interface_declaration" => Some("interface"),
                "type_alias_declaration" => Some("type"),
                "import_statement" | "import_declaration" => Some("import"),
                "export_statement" => Some("export"),
                "lexical_declaration" | "variable_declaration" => Some("variable"),
                "method_definition" => Some("method"),
                _ => None,
            }
        }

        fn extract_name(node: &tree_sitter::Node, source: &str) -> String {
            // Special handling for declarations where name is in child
            match node.kind() {
                "lexical_declaration" | "variable_declaration" => {
                    // Look for variable_declarator children with name field
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "variable_declarator" {
                                if let Some(name_node) = child.child_by_field_name("name") {
                                    if let Ok(text) = name_node.utf8_text(source.as_bytes()) {
                                        return text.to_string();
                                    }
                                }
                            }
                        }
                    }
                    "variable".to_string()
                }
                "import_statement" | "import_declaration" => {
                    // Try "source" for import source path, or first identifier
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            if child.kind() == "string" {
                                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                                    return text.to_string();
                                }
                            }
                            if child.kind() == "identifier" {
                                if let Ok(text) = child.utf8_text(source.as_bytes()) {
                                    return text.to_string();
                                }
                            }
                        }
                    }
                    "import".to_string()
                }
                _ => {
                    // Standard: try "name" field or first identifier child
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

                // Skip ERROR nodes (TypeScript syntax not parsed by JS)
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

                        // Update scope when descending into classes/functions
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
    }

    impl LanguageAdapter for TypeScriptAdapter {
        fn parse_source(&self, source: &str) -> Result<ParsedFile> {
            let (_, source_str) = self.parse_tree(source)?;
            Ok(ParsedFile {
                language: "typescript".to_string(),
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
            Box::new(TypeScriptAdapter::new())
        }
    }

    pub fn register_to(registry: &crate::app::bootstrap::Registry) {
        crate::register_language_adapter!(registry, "typescript", TypeScriptAdapter::new());
        crate::register_language_adapter!(registry, "javascript", TypeScriptAdapter::new());
    }
}

#[cfg(not(feature = "parsing"))]
mod typescript_adapter {
    // stub implementation when parsing feature not enabled
}

pub use typescript_adapter::*;
