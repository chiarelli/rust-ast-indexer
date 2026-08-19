#[cfg(feature = "parsing")]
mod python_adapter {
    use crate::adapters::LanguageAdapter;
    use crate::domain::parser::ParsedFile;
    use crate::domain::types::Symbol;
    use anyhow::Result;
    use tree_sitter::{Parser, Tree};

    fn python_language() -> tree_sitter::Language {
        tree_sitter_python::language()
    }

    pub struct PythonAdapter;

    impl Default for PythonAdapter {
        fn default() -> Self {
            Self
        }
    }

    impl PythonAdapter {
        pub fn new() -> Self {
            PythonAdapter
        }

        fn parse_tree(&self, source: &str) -> Result<(Tree, String)> {
            let mut parser = Parser::new();
            parser
                .set_language(python_language())
                .map_err(|e| anyhow::anyhow!("set_language failed: {:?}", e))?;
            let tree = parser
                .parse(source, None)
                .ok_or_else(|| anyhow::anyhow!("parse failed"))?;
            Ok((tree, source.to_string()))
        }

        fn node_type(kind: &str) -> Option<&str> {
            match kind {
                "function_definition" => Some("function"),
                "class_definition" => Some("class"),
                "decorated_definition" => Some("decorated"),
                "lambda" => Some("function"),
                "assignment" => Some("variable"),
                "import_statement" | "import_from_statement" => Some("import"),
                "async_function_definition" => Some("function"),
                _ => None,
            }
        }

        fn extract_name(node: &tree_sitter::Node, source: &str) -> String {
            match node.kind() {
                "decorated_definition" => {
                    // The actual definition is the last child
                    for i in (0..node.child_count()).rev() {
                        if let Some(child) = node.child(i) {
                            let name = Self::extract_name(&child, source);
                            if name != "decorated" && name != "decorator" {
                                return name;
                            }
                        }
                    }
                    "decorated".to_string()
                }
                "import_statement" | "import_from_statement" => {
                    // Extract module name from import
                    // "import os" -> "os", "from os.path import join" -> "os.path"
                    for i in 0..node.child_count() {
                        if let Some(child) = node.child(i) {
                            match child.kind() {
                                "dotted_name" | "relative_import" => {
                                    if let Ok(text) = child.utf8_text(source.as_bytes()) {
                                        return text.to_string();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "import".to_string()
                }
                "assignment" => {
                    // Left side of assignment is the variable name
                    if let Some(left) = node.child_by_field_name("left") {
                        if let Ok(text) = left.utf8_text(source.as_bytes()) {
                            // Handle tuple unpacking: just take the full text
                            return text.to_string();
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

                    // Update scope when descending into classes/functions
                    let new_scope = name.clone();
                    if cursor.goto_first_child() {
                        Self::walk_tree(cursor, source, file_path, Some(&new_scope), symbols);
                        cursor.goto_parent();
                    }
                } else if kind == "ERROR" {
                    // Skip ERROR nodes silently
                    // Just descend to see if children have valid nodes
                    if cursor.goto_first_child() {
                        Self::walk_tree(cursor, source, file_path, scope, symbols);
                        cursor.goto_parent();
                    }
                } else if node.child_count() > 0 && cursor.goto_first_child() {
                    Self::walk_tree(cursor, source, file_path, scope, symbols);
                    cursor.goto_parent();
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        // Collect import edges by traversing the tree
        fn collect_imports(
            cursor: &mut tree_sitter::TreeCursor,
            source: &str,
            file_path: &str,
            edges: &mut Vec<crate::domain::types::ImportEdge>,
        ) {
            loop {
                let node = cursor.node();
                let kind = node.kind();

                if kind == "import_statement" || kind == "import_from_statement" {
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

        // Collect call edges by traversing the tree
        fn collect_calls(
            cursor: &mut tree_sitter::TreeCursor,
            source: &str,
            file_path: &str,
            edges: &mut Vec<crate::domain::types::CallEdge>,
        ) {
            loop {
                let node = cursor.node();
                let kind = node.kind();

                if kind == "call" {
                    let start = node.start_position();
                    let end = node.end_position();

                    // Try to get callee as the function child (first child by field or position)
                    let callee = node
                        .child_by_field_name("function")
                        .or_else(|| node.child(0))
                        .and_then(|c| c.utf8_text(source.as_bytes()).ok())
                        .map(|s| s.to_string())
                        .unwrap_or_default();

                    // Determine caller by searching ancestors
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

                    let call_kind = if callee.contains('.') { "dynamic" } else { "static" };

                    let edge = crate::domain::types::CallEdge {
                        id: format!("ce:{}:{}:{}", file_path, start.row, start.column),
                        caller_symbol_id: caller_id,
                        callee_name: callee,
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

    impl LanguageAdapter for PythonAdapter {
        fn parse_source(&self, source: &str) -> Result<ParsedFile> {
            let (_, _source_str) = self.parse_tree(source)?;
            Ok(ParsedFile {
                language: "python".to_string(),
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
            PythonAdapter::extract_imports(self, parsed)
        }

        fn extract_calls(
            &self,
            parsed: &ParsedFile,
        ) -> Result<Vec<crate::domain::types::CallEdge>> {
            PythonAdapter::extract_calls(self, parsed)
        }

        fn box_clone(&self) -> Box<dyn LanguageAdapter> {
            Box::new(PythonAdapter::new())
        }
    }

    pub fn register_to(registry: &crate::app::bootstrap::Registry) {
        crate::register_language_adapter!(registry, "python", PythonAdapter::new());
    }
}

#[cfg(not(feature = "parsing"))]
mod python_adapter {
    // stub implementation when parsing feature not enabled
}

#[cfg(feature = "parsing")]
pub use python_adapter::*;
