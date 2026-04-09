use std::collections::HashMap;

use crate::application::chunking::{apply_token_count, ChunkStrategy};
use crate::domain::types::{Chunk, Symbol};

pub struct SemanticChunker {
    pub max_lines: usize,
    semantic_gap_lines: usize,
}

impl SemanticChunker {
    pub fn new(max_lines: usize) -> Self {
        Self {
            max_lines,
            semantic_gap_lines: 3,
        }
    }
}

impl ChunkStrategy for SemanticChunker {
    fn chunk_file(&self, file_path: &str, source: &str, symbols: Option<&Vec<Symbol>>) -> Vec<Chunk> {
        let Some(symbols) = symbols else {
            return vec![build_full_file_chunk(file_path, source, self.max_lines)];
        };

        if symbols.is_empty() {
            return vec![build_full_file_chunk(file_path, source, self.max_lines)];
        }

        let mut sorted_symbols = symbols.clone();
        sorted_symbols.sort_by_key(|symbol| (symbol.start_line, symbol.end_line, symbol.id.clone()));

        let mut chunks = Vec::new();
        let mut current_group: Vec<Symbol> = Vec::new();
        let mut current_key: Option<String> = None;
        let mut current_anchor_end: Option<usize> = None;
        let mut current_start = 0usize;
        let mut current_end = 0usize;

        for symbol in sorted_symbols {
            let symbol_key = semantic_key(&symbol).or_else(|| {
                if current_group.is_empty() {
                    None
                } else if symbol.start_line >= current_start && symbol.end_line <= current_end {
                    current_key.clone()
                } else {
                    None
                }
            });

            if current_group.is_empty() {
                current_start = symbol.start_line;
                current_end = symbol.end_line;
                current_anchor_end = is_anchor_symbol(&symbol).then_some(symbol.end_line);
                current_key = symbol_key.or_else(|| Some(fallback_key(&symbol)));
                current_group.push(symbol);
                continue;
            }

            let next_end = current_end.max(symbol.end_line);
            let next_lines = next_end.saturating_sub(current_start) + 1;
            let same_group = current_key.as_ref().zip(symbol_key.as_ref()).map_or(false, |(a, b)| a == b);
            let close_enough = symbol.start_line <= current_end.saturating_add(self.semantic_gap_lines);
            let fits_limit = self.max_lines == 0 || next_lines <= self.max_lines;
            let within_anchor = current_anchor_end.map_or(false, |anchor_end| {
                is_member_symbol(&symbol) && symbol.end_line <= anchor_end
            });

            if (same_group || within_anchor) && close_enough && fits_limit {
                current_end = next_end;
                current_anchor_end = current_anchor_end.map(|anchor_end| anchor_end.max(symbol.end_line));
                current_group.push(symbol);
            } else {
                flush_group(
                    file_path,
                    source,
                    &mut chunks,
                    std::mem::take(&mut current_group),
                    current_start,
                    current_end,
                    current_key.as_deref(),
                    self.max_lines,
                );
                current_start = symbol.start_line;
                current_end = symbol.end_line;
                current_anchor_end = is_anchor_symbol(&symbol).then_some(symbol.end_line);
                current_key = symbol_key.or_else(|| Some(fallback_key(&symbol)));
                current_group.push(symbol);
            }
        }

        if !current_group.is_empty() {
            flush_group(
                file_path,
                source,
                &mut chunks,
                current_group,
                current_start,
                current_end,
                current_key.as_deref(),
                self.max_lines,
            );
        }

        chunks
    }
}

fn build_full_file_chunk(file_path: &str, source: &str, max_lines: usize) -> Chunk {
    let content = source.to_string();
    let mut metadata = HashMap::from([
        ("chunk_strategy".to_string(), serde_json::Value::String("semantic".to_string())),
        (
            "max_line_limit".to_string(),
            serde_json::Value::Number(serde_json::Number::from(max_lines as u64)),
        ),
    ]);
    apply_token_count(&mut metadata, &content);

    Chunk {
        id: format!("chk-sem-{}", blake3::hash(content.as_bytes()).to_hex()),
        file_path: file_path.to_string(),
        start_line: 1,
        end_line: source.lines().count().max(1),
        content: content.clone(),
        text: content.clone(),
        md5: blake3::hash(content.as_bytes()).to_hex().to_string(),
        size: content.len(),
        language: None,
        symbol_id: None,
        symbol_ids: vec![],
        chunk_kind: Some("FullFile".into()),
        metadata: Some(metadata),
    }
}

fn flush_group(
    file_path: &str,
    source: &str,
    chunks: &mut Vec<Chunk>,
    group: Vec<Symbol>,
    start_line: usize,
    end_line: usize,
    group_key: Option<&str>,
    max_lines: usize,
) {
    if group.is_empty() {
        return;
    }

    let content = lines_to_string(source, start_line, end_line);
    let symbol_ids = group.iter().map(|symbol| symbol.id.clone()).collect::<Vec<_>>();
    let symbol_id = symbol_ids.first().cloned();
    let mut metadata = HashMap::from([
        ("chunk_strategy".to_string(), serde_json::Value::String("semantic".to_string())),
        (
            "max_line_limit".to_string(),
            serde_json::Value::Number(serde_json::Number::from(max_lines as u64)),
        ),
    ]);

    if let Some(key) = group_key {
        metadata.insert("semantic_key".to_string(), serde_json::Value::String(key.to_string()));
    }

    metadata.insert(
        "symbol_count".to_string(),
        serde_json::Value::Number(serde_json::Number::from(symbol_ids.len() as u64)),
    );
    apply_token_count(&mut metadata, &content);

    let digest_input = format!("{}:{}:{}:{}", file_path, start_line, end_line, symbol_ids.join("|"));
    let digest = blake3::hash(digest_input.as_bytes()).to_hex().to_string();

    chunks.push(Chunk {
        id: format!("chk-sem-{}", digest),
        file_path: file_path.to_string(),
        start_line,
        end_line,
        content: content.clone(),
        text: content.clone(),
        md5: digest,
        size: content.len(),
        language: None,
        symbol_id,
        symbol_ids,
        chunk_kind: Some("Symbol".into()),
        metadata: Some(metadata),
    });
}

fn lines_to_string(source: &str, start: usize, end: usize) -> String {
    source
        .lines()
        .enumerate()
        .filter_map(|(idx, line)| {
            let line_number = idx + 1;
            if (start..=end).contains(&line_number) {
                Some(line)
            } else {
                None
            }
        })
        .map(|line| {
            let mut out = String::with_capacity(line.len() + 1);
            out.push_str(line);
            out.push('\n');
            out
        })
        .collect()
}

fn semantic_key(symbol: &Symbol) -> Option<String> {
    let kind = symbol.kind.to_lowercase();
    match kind.as_str() {
        "struct" | "class" | "trait" | "enum" | "type" | "mod" => Some(normalize_name(&symbol.name)),
        "impl" => extract_impl_target(symbol.signature.as_deref().unwrap_or(&symbol.name))
            .or_else(|| Some(normalize_name(&symbol.name))),
        "method" | "constructor" | "field" => symbol.scope.as_deref().map(scope_key),
        "function" => symbol
            .scope
            .as_deref()
            .map(scope_key)
            .or_else(|| Some(format!("fn:{}", normalize_name(&symbol.name)))),
        _ => symbol.scope.as_deref().map(scope_key),
    }
}

fn fallback_key(symbol: &Symbol) -> String {
    format!("{}:{}", symbol.kind.to_lowercase(), normalize_name(&symbol.name))
}

fn normalize_name(value: &str) -> String {
    value.trim().trim_end_matches("::").to_lowercase()
}

fn scope_key(scope: &str) -> String {
    normalize_name(scope.split("::").last().unwrap_or(scope))
}

fn extract_impl_target(signature: &str) -> Option<String> {
    let signature = signature.trim();
    let signature = signature.strip_prefix("impl")?.trim_start();
    let signature = signature.split_once('{').map(|(head, _)| head).unwrap_or(signature);
    let signature = signature.split_once(" where ").map(|(head, _)| head).unwrap_or(signature);
    let target = signature
        .rfind(" for ")
        .map(|idx| &signature[idx + 5..])
        .unwrap_or(signature)
        .trim();

    target
        .split_whitespace()
        .next()
        .map(|value| value.trim_matches(|ch: char| matches!(ch, '<' | '>' | '{' | '}' | '(' | ')' | ',' | ';')).to_lowercase())
}

fn is_anchor_symbol(symbol: &Symbol) -> bool {
    matches!(symbol.kind.to_lowercase().as_str(), "struct" | "class" | "trait" | "enum" | "type" | "mod" | "impl")
}

fn is_member_symbol(symbol: &Symbol) -> bool {
    matches!(symbol.kind.to_lowercase().as_str(), "method" | "constructor" | "function" | "field")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::chunking::ChunkStrategy;

    fn symbol(
        id: &str,
        name: &str,
        kind: &str,
        scope: Option<&str>,
        signature: Option<&str>,
        start_line: usize,
        end_line: usize,
    ) -> Symbol {
        Symbol {
            id: id.into(),
            name: name.into(),
            kind: kind.into(),
            scope: scope.map(|value| value.into()),
            file_path: "src/lib.rs".into(),
            start_line,
            end_line,
            signature: signature.map(|value| value.into()),
        }
    }

    #[test]
    fn semantic_chunker_groups_struct_impl_and_methods() {
        let source = "pub struct UserService {\n    repo: Repo,\n}\n\nimpl UserService {\n    pub fn new() -> Self {\n        Self { repo: Repo::new() }\n    }\n\n    pub fn add(&self) {\n        println!(\"add\");\n    }\n}\n\nfn unrelated() {}\n";
        let symbols = vec![
            symbol("sym::user_service", "UserService", "struct", None, None, 1, 3),
            symbol("sym::user_service_impl", "UserService", "impl", None, Some("impl UserService {"), 5, 12),
            symbol("sym::new", "new", "method", Some("UserService"), None, 6, 8),
            symbol("sym::add", "add", "method", Some("UserService"), None, 10, 12),
            symbol("sym::unrelated", "unrelated", "function", None, None, 15, 15),
        ];

        let chunker = SemanticChunker::new(0);
        let chunks = chunker.chunk_file("src/lib.rs", source, Some(&symbols));

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].symbol_ids.len(), 4);
        assert!(chunks[0].content.contains("pub struct UserService"));
        assert!(chunks[0].content.contains("pub fn add"));
        assert_eq!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("chunk_strategy")), Some(&serde_json::Value::String("semantic".to_string())));
        assert_eq!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("max_line_limit")).and_then(|value| value.as_u64()), Some(0));
        assert_eq!(chunks[1].symbol_ids, vec!["sym::unrelated".to_string()]);
    }

    #[test]
    fn semantic_chunker_keeps_unrelated_symbols_separate() {
        let source = "fn a() {}\n\nfn b() {}\n";
        let symbols = vec![
            symbol("sym::a", "a", "function", None, None, 1, 1),
            symbol("sym::b", "b", "function", None, None, 3, 3),
        ];

        let chunker = SemanticChunker::new(0);
        let chunks = chunker.chunk_file("src/lib.rs", source, Some(&symbols));

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].symbol_ids, vec!["sym::a".to_string()]);
        assert_eq!(chunks[1].symbol_ids, vec!["sym::b".to_string()]);
    }

    #[test]
    fn semantic_chunker_uses_scope_for_class_methods() {
        let source = "class Database {\n    connect() { return true; }\n\n    async query(sql) {\n        return [];\n    }\n}\n";
        let symbols = vec![
            symbol("sym::database", "Database", "class", None, None, 1, 7),
            symbol("sym::connect", "connect", "method", Some("Database"), None, 2, 2),
            symbol("sym::query", "query", "method", Some("Database"), None, 4, 6),
        ];

        let chunker = SemanticChunker::new(0);
        let chunks = chunker.chunk_file("src/lib.ts", source, Some(&symbols));

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol_ids, vec!["sym::database".to_string(), "sym::connect".to_string(), "sym::query".to_string()]);
        assert_eq!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("semantic_key")), Some(&serde_json::Value::String("database".to_string())));
        assert_eq!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("max_line_limit")).and_then(|value| value.as_u64()), Some(0));
    }

    #[test]
    fn semantic_chunker_splits_when_line_limit_is_exceeded() {
        let source = "class Database {\n    connect() { return true; }\n\n    async query(sql) {\n        return [];\n    }\n}\n";
        let symbols = vec![
            symbol("sym::database", "Database", "class", None, None, 1, 7),
            symbol("sym::connect", "connect", "method", Some("Database"), None, 2, 2),
            symbol("sym::query", "query", "method", Some("Database"), None, 4, 6),
        ];

        let chunker = SemanticChunker::new(4);
        let chunks = chunker.chunk_file("src/lib.ts", source, Some(&symbols));

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].symbol_ids, vec!["sym::database".to_string()]);
        assert_eq!(chunks[1].symbol_ids, vec!["sym::connect".to_string()]);
        assert_eq!(chunks[2].symbol_ids, vec!["sym::query".to_string()]);
        assert_eq!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("max_line_limit")).and_then(|value| value.as_u64()), Some(4));
    }
}
