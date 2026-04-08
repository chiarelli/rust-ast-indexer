use std::collections::{BTreeSet, HashSet};

use crate::application::chunking::ChunkStrategy;
use crate::domain::types::{Chunk, Symbol};

pub struct ContextInjectionChunker<S> {
    inner: S,
}

impl<S> ContextInjectionChunker<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S: ChunkStrategy> ChunkStrategy for ContextInjectionChunker<S> {
    fn chunk_file(&self, file_path: &str, source: &str, symbols: Option<&Vec<Symbol>>) -> Vec<Chunk> {
        let chunks = self.inner.chunk_file(file_path, source, symbols);
        let symbol_index = symbols.map(|items| index_symbols_by_id(items.as_slice()));

        chunks
            .into_iter()
            .map(|chunk| inject_context(chunk, source, symbol_index.as_ref()))
            .collect()
    }
}

fn inject_context(
    mut chunk: Chunk,
    source: &str,
    symbol_index: Option<&std::collections::HashMap<String, Symbol>>,
) -> Chunk {
    let imports = extract_leading_imports(source);
    let scopes = collect_scopes(&chunk, symbol_index);

    let context = build_context_block(&imports, &scopes);
    if context.is_empty() {
        return chunk;
    }

    let content = format!("{}{}", context, chunk.content);
    chunk.size = content.len();
    chunk.md5 = blake3::hash(content.as_bytes()).to_hex().to_string();
    chunk.content = content.clone();
    chunk.text = content;
    chunk.metadata = merge_metadata(chunk.metadata.take(), imports, scopes);
    chunk
}

fn index_symbols_by_id(symbols: &[Symbol]) -> std::collections::HashMap<String, Symbol> {
    symbols.iter().cloned().map(|symbol| (symbol.id.clone(), symbol)).collect()
}

fn extract_leading_imports(source: &str) -> Vec<String> {
    let mut imports = Vec::new();
    let mut seen_import = false;
    let mut seen = HashSet::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if seen_import {
                imports.push(String::new());
            }
            continue;
        }

        if is_import_line(trimmed) {
            seen_import = true;
            if seen.insert(trimmed.to_string()) {
                imports.push(trimmed.to_string());
            }
            continue;
        }

        if seen_import {
            break;
        }
    }

    imports
}

fn is_import_line(line: &str) -> bool {
    line.starts_with("use ") || line.starts_with("import ")
}

fn collect_scopes(
    chunk: &Chunk,
    symbol_index: Option<&std::collections::HashMap<String, Symbol>>,
) -> Vec<String> {
    let mut scopes = BTreeSet::new();

    if let Some(symbol_id) = &chunk.symbol_id {
        if let Some(symbol) = symbol_index.and_then(|index| index.get(symbol_id)) {
            if let Some(scope) = &symbol.scope {
                scopes.insert(scope.trim().to_string());
            }
        }
    }

    for symbol_id in &chunk.symbol_ids {
        if let Some(symbol) = symbol_index.and_then(|index| index.get(symbol_id)) {
            if let Some(scope) = &symbol.scope {
                scopes.insert(scope.trim().to_string());
            }
        }
    }

    scopes.into_iter().collect()
}

fn build_context_block(imports: &[String], scopes: &[String]) -> String {
    let mut lines = Vec::new();

    if !imports.is_empty() {
        lines.extend(imports.iter().cloned());
    }

    if !scopes.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }

        lines.extend(scopes.iter().map(|scope| format!("scope: {}", scope)));
    }

    if lines.is_empty() {
        return String::new();
    }

    let mut block = lines.join("\n");
    block.push_str("\n\n");
    block
}

fn merge_metadata(
    metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
    imports: Vec<String>,
    scopes: Vec<String>,
) -> Option<std::collections::HashMap<String, serde_json::Value>> {
    let mut metadata = metadata.unwrap_or_default();
    metadata.insert(
        "has_context_prefix".into(),
        serde_json::Value::Bool(!imports.is_empty() || !scopes.is_empty()),
    );
    metadata.insert(
        "context_import_count".into(),
        serde_json::Value::Number(serde_json::Number::from(imports.iter().filter(|line| !line.is_empty()).count() as u64)),
    );
    metadata.insert(
        "context_scope_count".into(),
        serde_json::Value::Number(serde_json::Number::from(scopes.len() as u64)),
    );
    Some(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::chunking::ChunkStrategy;
    use crate::domain::types::Chunk;

    struct StubChunker;

    impl ChunkStrategy for StubChunker {
        fn chunk_file(&self, file_path: &str, _source: &str, _symbols: Option<&Vec<Symbol>>) -> Vec<Chunk> {
            vec![Chunk {
                id: format!("chk-{}", file_path),
                file_path: file_path.into(),
                start_line: 3,
                end_line: 5,
                content: "fn inner() {}".into(),
                text: "fn inner() {}".into(),
                md5: "base".into(),
                size: 13,
                language: Some("rust".into()),
                symbol_id: Some("sym::inner".into()),
                symbol_ids: vec!["sym::inner".into()],
                chunk_kind: Some("Symbol".into()),
                metadata: None,
            }]
        }
    }

    fn symbol(id: &str, scope: Option<&str>) -> Symbol {
        Symbol {
            id: id.into(),
            name: id.into(),
            kind: "function".into(),
            scope: scope.map(|value| value.into()),
            file_path: "src/lib.rs".into(),
            start_line: 3,
            end_line: 5,
            signature: None,
        }
    }

    #[test]
    fn injects_import_prefix() {
        let source = "use crate::foo;\nuse crate::bar;\n\nfn inner() {}\n";
        let decorator = ContextInjectionChunker::new(StubChunker);
        let chunks = decorator.chunk_file("src/lib.rs", source, Some(&vec![symbol("sym::inner", None)]));

        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.starts_with("use crate::foo;\nuse crate::bar;\n\n"));
        assert_eq!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("has_context_prefix")), Some(&serde_json::Value::Bool(true)));
    }

    #[test]
    fn preserves_scope_chain_in_prefix() {
        let source = "use crate::foo;\n\nfn inner() {}\n";
        let decorator = ContextInjectionChunker::new(StubChunker);
        let chunks = decorator.chunk_file("src/lib.rs", source, Some(&vec![symbol("sym::inner", Some("services::user::UserService"))]));

        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("scope: services::user::UserService"));
        assert!(chunks[0].content.contains("use crate::foo;"));
    }

    #[test]
    fn avoids_duplicate_context_lines() {
        let source = "use crate::foo;\nuse crate::foo;\n\nfn inner() {}\n";
        let decorator = ContextInjectionChunker::new(StubChunker);
        let chunks = decorator.chunk_file("src/lib.rs", source, Some(&vec![symbol("sym::inner", Some("svc"))]));

        let prefix = chunks[0].content.lines().take_while(|line| !line.is_empty()).collect::<Vec<_>>();
        let foo_count = prefix.iter().filter(|line| **line == "use crate::foo;").count();
        assert_eq!(foo_count, 1);
    }
}
