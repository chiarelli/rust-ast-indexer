#[cfg(feature = "parsing")]
mod java_adapter {
    use crate::adapters::LanguageAdapter;
    use crate::domain::parser::ParsedFile;
    use crate::domain::types::Symbol;
    use anyhow::Result;
    use tree_sitter::{Parser, Tree};

    fn java_language() -> tree_sitter::Language {
        tree_sitter_java::language()
    }

    pub struct JavaAdapter;

    impl Default for JavaAdapter {
        fn default() -> Self { Self }
    }

    impl JavaAdapter {
        pub fn new() -> Self { JavaAdapter }

        fn parse_tree(&self, source: &str) -> Result<(Tree, String)> {
            let mut parser = Parser::new();
            parser
                .set_language(java_language())
                .map_err(|e| anyhow::anyhow!("set_language failed: {:?}", e))?;
            let tree = parser
                .parse(source, None)
                .ok_or_else(|| anyhow::anyhow!("parse failed"))?;
            Ok((tree, source.to_string()))
        }

        fn node_type(kind: &str) -> Option<&str> {
            match kind {
                "method_declaration" => Some("method"),
                "class_declaration" => Some("class"),
                "enum_declaration" => Some("enum"),
                "interface_declaration" => Some("interface"),
                "annotation_type_declaration" => Some("annotation"),
                "constructor_declaration" => Some("constructor"),
                "import_declaration" => Some("import"),
                "field_declaration" => Some("field"),
                _ => None,
            }
        }

        fn extract_name(node: &tree_sitter::Node, source: &str) -> String {
            match node.kind() {
                "field_declaration" => {
                    // Field names are inside declarators
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
                    "field".to_string()
                }
                _ => {
                    // Standard: try "name" field first
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

                        // Update scope when descending into classes/methods
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
            edges: &mut Vec<crate::domain::types::ImportEdge>,
        ) {
            loop {
                let node = cursor.node();
                let kind = node.kind();

                if kind == "import_declaration" {
                    let start = node.start_position();
                    let end = node.end_position();
                    let text = node.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()).unwrap_or_default();
                    let edge = crate::domain::types::ImportEdge {
                        id: format!("ie:{}:{}:{}", file_path, start.row, start.column),
                        from_file: file_path.to_string(),
                        to_module: text.trim().to_string(),
                        imported_symbol: None,
                        alias: None,
                        import_kind: "named".to_string(),
                        location: crate::domain::types::Location { start_line: start.row, start_col: start.column, end_line: end.row, end_col: end.column },
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

        pub fn extract_imports(&self, parsed: &crate::domain::parser::ParsedFile) -> Result<Vec<crate::domain::types::ImportEdge>> {
            let (tree, _) = self.parse_tree(&parsed.source)?;
            let mut cursor = tree.walk();
            let mut edges = Vec::new();
            Self::collect_imports(&mut cursor, &parsed.source, "<source>", &mut edges);
            Ok(edges)
        }

        // Collect call edges by traversing the tree and matching "method_invocation" nodes
        fn collect_calls(
            cursor: &mut tree_sitter::TreeCursor,
            source: &str,
            file_path: &str,
            edges: &mut Vec<crate::domain::types::CallEdge>,
        ) {
            loop {
                let node = cursor.node();
                let kind = node.kind();

                if kind == "method_invocation" {
                    let start = node.start_position();
                    let end = node.end_position();
                    // For method invocations, extract full node text and derive the last identifier before '('
                    let full_text = node.utf8_text(source.as_bytes()).unwrap_or_default().to_string();
                    let before_paren = if let Some(pos) = full_text.find('(') { full_text[..pos].to_string() } else { full_text.clone() };
                    // find last identifier run (letters, digits, underscore)
                    let mut callee = String::new();
                    if !before_paren.is_empty() {
                        let bytes = before_paren.as_bytes();
                        let mut i = bytes.len();
                        while i > 0 {
                            i -= 1;
                            let ch = bytes[i] as char;
                            if ch.is_ascii_alphanumeric() || ch == '_' {
                                // find start of run
                                let mut start = i;
                                while start > 0 {
                                    let pc = bytes[start - 1] as char;
                                    if pc.is_ascii_alphanumeric() || pc == '_' { start -= 1 } else { break; }
                                }
                                if start <= i {
                                    callee = before_paren[start..=i].to_string();
                                }
                                break;
                            }
                        }
                    }
                    if callee.is_empty() {
                        // fallback: last path segment after '.' or ':'
                        if before_paren.contains('.') || before_paren.contains(':') {
                            if let Some(last) = before_paren.split(|c: char| c=='.' || c==':').last() {
                                callee = last.trim().to_string();
                            }
                        } else {
                            callee = before_paren.trim().to_string();
                        }
                    }
                    callee = callee.trim().to_string();

                    // Determine caller by searching ancestors for a method/class-like node
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

                    // simple heuristic for call kind - in Java we can check if it's a constructor call or regular method
                    let call_kind = "static"; // conservative default, could be refined

                    let edge = crate::domain::types::CallEdge {
                        id: format!("ce:{}:{}:{}", file_path, start.row, start.column),
                        caller_symbol_id: caller_id,
                        callee_name: callee.clone(),
                        callee_symbol_id: None,
                        call_kind: call_kind.to_string(),
                        location: crate::domain::types::Location { start_line: start.row, start_col: start.column, end_line: end.row, end_col: end.column },
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

        pub fn extract_calls(&self, parsed: &crate::domain::parser::ParsedFile) -> Result<Vec<crate::domain::types::CallEdge>> {
            let (tree, _) = self.parse_tree(&parsed.source)?;
            let mut cursor = tree.walk();
            let mut edges = Vec::new();
            Self::collect_calls(&mut cursor, &parsed.source, "<source>", &mut edges);
            Ok(edges)
        }
    }

    impl LanguageAdapter for JavaAdapter {
        fn parse_source(&self, source: &str) -> Result<ParsedFile> {
            let (_, source_str) = self.parse_tree(source)?;
            Ok(ParsedFile {
                language: "java".to_string(),
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
            Box::new(JavaAdapter::new())
        }
    }

    pub fn register_to(registry: &crate::app::bootstrap::Registry) {
        crate::register_language_adapter!(registry, "java", JavaAdapter::new());
    }
}

#[cfg(not(feature = "parsing"))]
mod java_adapter {
    // stub implementation when parsing feature not enabled
}

pub use java_adapter::*;
