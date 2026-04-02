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

    impl RustAdapter {
        pub fn new() -> Self { RustAdapter }

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
                }

                let has_children = node.child_count() > 0;
                let should_descend = has_children;
                if should_descend && cursor.goto_first_child() {
                    Self::walk_tree(
                        cursor,
                        source,
                        file_path,
                        scope,
                        symbols,
                    );
                    cursor.goto_parent();
                }

                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    impl LanguageAdapter for RustAdapter {
        fn parse_source(&self, source: &str) -> Result<ParsedFile> {
            let (_, source_str) = self.parse_tree(source)?;
            Ok(ParsedFile {
                language: "rust".to_string(),
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

pub use rust_adapter::*;
