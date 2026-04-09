use std::collections::HashMap;

use crate::application::chunking::ChunkStrategy;
use crate::domain::types::{Chunk, Symbol};

pub struct ApproxTokenLimitedChunker {
    pub max_tokens: usize,
}

impl ApproxTokenLimitedChunker {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }
}

impl ChunkStrategy for ApproxTokenLimitedChunker {
    fn chunk_file(&self, file_path: &str, source: &str, symbols: Option<&Vec<Symbol>>) -> Vec<Chunk> {
        let Some(symbols) = symbols else {
            return vec![build_full_file_chunk(file_path, source)];
        };

        if symbols.is_empty() {
            return vec![build_full_file_chunk(file_path, source)];
        }

        let mut sorted_symbols = symbols.clone();
        sorted_symbols.sort_by_key(|symbol| (symbol.start_line, symbol.end_line, symbol.id.clone()));

        let mut chunks = Vec::new();
        let mut current_group: Vec<Symbol> = Vec::new();
        let mut current_start = 0usize;
        let mut current_end = 0usize;

        for symbol in sorted_symbols {
            let symbol_tokens = approximate_tokens(&lines_to_string(source, symbol.start_line, symbol.end_line));
            let symbol_over_limit = self.max_tokens > 0 && symbol_tokens > self.max_tokens;

            if current_group.is_empty() {
                current_start = symbol.start_line;
                current_end = symbol.end_line;
                current_group.push(symbol);
                if symbol_over_limit {
                    flush_group(file_path, source, &mut chunks, std::mem::take(&mut current_group), current_start, current_end, self.max_tokens);
                }
                continue;
            }

            let next_end = current_end.max(symbol.end_line);
            let next_tokens = approximate_tokens(&lines_to_string(source, current_start, next_end));

            if self.max_tokens > 0 && next_tokens > self.max_tokens {
                flush_group(file_path, source, &mut chunks, std::mem::take(&mut current_group), current_start, current_end, self.max_tokens);
                current_start = symbol.start_line;
                current_end = symbol.end_line;
                current_group.push(symbol);
                if symbol_over_limit {
                    flush_group(file_path, source, &mut chunks, std::mem::take(&mut current_group), current_start, current_end, self.max_tokens);
                }
            } else {
                current_end = next_end;
                current_group.push(symbol);
            }
        }

        if !current_group.is_empty() {
            flush_group(file_path, source, &mut chunks, current_group, current_start, current_end, self.max_tokens);
        }

        chunks
    }
}

fn build_full_file_chunk(file_path: &str, source: &str) -> Chunk {
    let content = source.to_string();
    let digest = blake3::hash(content.as_bytes()).to_hex().to_string();
    Chunk {
        id: format!("chk-tok-{}", digest),
        file_path: file_path.to_string(),
        start_line: 1,
        end_line: source.lines().count().max(1),
        content: content.clone(),
        text: content.clone(),
        md5: digest.clone(),
        size: content.len(),
        language: None,
        symbol_id: None,
        symbol_ids: vec![],
        chunk_kind: Some("FullFile".into()),
        metadata: Some(HashMap::from([
            ("chunk_strategy".to_string(), serde_json::Value::String("token".to_string())),
            (
                "approx_token_count".to_string(),
                serde_json::Value::Number(serde_json::Number::from(approximate_tokens(&content) as u64)),
            ),
        ])),
    }
}

fn flush_group(
    file_path: &str,
    source: &str,
    chunks: &mut Vec<Chunk>,
    group: Vec<Symbol>,
    start_line: usize,
    end_line: usize,
    max_tokens: usize,
) {
    if group.is_empty() {
        return;
    }

    let content = lines_to_string(source, start_line, end_line);
    let symbol_ids = group.iter().map(|symbol| symbol.id.clone()).collect::<Vec<_>>();
    let symbol_id = symbol_ids.first().cloned();
    let token_count = approximate_tokens(&content);
    let digest_input = format!("{}:{}:{}:{}", file_path, start_line, end_line, symbol_ids.join("|"));
    let digest = blake3::hash(digest_input.as_bytes()).to_hex().to_string();

    chunks.push(Chunk {
        id: format!("chk-tok-{}", digest),
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
        metadata: Some(HashMap::from([
            ("chunk_strategy".to_string(), serde_json::Value::String("token".to_string())),
            (
                "approx_token_count".to_string(),
                serde_json::Value::Number(serde_json::Number::from(token_count as u64)),
            ),
            (
                "max_token_limit".to_string(),
                serde_json::Value::Number(serde_json::Number::from(max_tokens as u64)),
            ),
        ])),
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

fn approximate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::chunking::ChunkStrategy;

    fn symbol(id: &str, start_line: usize, end_line: usize) -> Symbol {
        Symbol {
            id: id.into(),
            name: id.into(),
            kind: "function".into(),
            scope: None,
            file_path: "src/lib.rs".into(),
            start_line,
            end_line,
            signature: None,
        }
    }

    #[test]
    fn groups_symbols_until_token_limit_is_reached() {
        let source = "fn a() {}\nfn b() {}\nfn c() {}\n";
        let chunker = ApproxTokenLimitedChunker::new(5);
        let chunks = chunker.chunk_file(
            "src/lib.rs",
            source,
            Some(&vec![symbol("sym::a", 1, 1), symbol("sym::b", 2, 2), symbol("sym::c", 3, 3)]),
        );

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].symbol_ids, vec!["sym::a", "sym::b"]);
        assert_eq!(chunks[1].symbol_ids, vec!["sym::c"]);
        assert_eq!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("chunk_strategy")), Some(&serde_json::Value::String("token".to_string())));
    }

    #[test]
    fn keeps_oversized_symbol_in_its_own_chunk() {
        let source = "fn big() { println!(\"a\"); println!(\"b\"); println!(\"c\"); }\n";
        let chunker = ApproxTokenLimitedChunker::new(4);
        let chunks = chunker.chunk_file("src/lib.rs", source, Some(&vec![symbol("sym::big", 1, 1)]));

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol_ids, vec!["sym::big"]);
        assert!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("approx_token_count")).is_some());
        assert!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("approx_token_count")).and_then(|value| value.as_u64()).unwrap_or_default() > 4);
    }

    #[test]
    fn falls_back_to_full_file_without_symbols() {
        let source = "alpha\nbeta\n";
        let chunker = ApproxTokenLimitedChunker::new(8);
        let chunks = chunker.chunk_file("notes.txt", source, None);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_kind.as_deref(), Some("FullFile"));
        assert_eq!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("chunk_strategy")), Some(&serde_json::Value::String("token".to_string())));
    }
}
