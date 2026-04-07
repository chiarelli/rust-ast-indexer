use crate::application::chunking::ChunkStrategy;
use crate::domain::types::{Chunk, Symbol};

pub struct SizeLimitedChunker {
    pub max_lines: usize,
}

impl SizeLimitedChunker {
    pub fn new(max_lines: usize) -> Self {
        Self { max_lines }
    }
}

impl ChunkStrategy for SizeLimitedChunker {
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
            let symbol_lines = symbol_lines(&symbol);
            let symbol_over_limit = self.max_lines > 0 && symbol_lines > self.max_lines;

            if current_group.is_empty() {
                current_start = symbol.start_line;
                current_end = symbol.end_line;
                current_group.push(symbol);
                if symbol_over_limit {
                    flush_group(file_path, source, &mut chunks, std::mem::take(&mut current_group), current_start, current_end);
                }
                continue;
            }

            let group_lines = current_end.saturating_sub(current_start) + 1;
            let next_end = current_end.max(symbol.end_line);
            let next_lines = next_end.saturating_sub(current_start) + 1;

            if self.max_lines > 0 && (group_lines > self.max_lines || next_lines > self.max_lines) {
                flush_group(file_path, source, &mut chunks, std::mem::take(&mut current_group), current_start, current_end);
                current_start = symbol.start_line;
                current_end = symbol.end_line;
                current_group.push(symbol);
                if symbol_over_limit {
                    flush_group(file_path, source, &mut chunks, std::mem::take(&mut current_group), current_start, current_end);
                }
            } else {
                current_end = next_end;
                current_group.push(symbol);
            }
        }

        if !current_group.is_empty() {
            flush_group(file_path, source, &mut chunks, current_group, current_start, current_end);
        }

        chunks
    }
}

fn build_full_file_chunk(file_path: &str, source: &str) -> Chunk {
    let content = source.to_string();
    Chunk {
        id: format!("chk-{}", blake3::hash(content.as_bytes()).to_hex()),
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
        metadata: None,
    }
}

fn flush_group(
    file_path: &str,
    source: &str,
    chunks: &mut Vec<Chunk>,
    group: Vec<Symbol>,
    start_line: usize,
    end_line: usize,
) {
    if group.is_empty() {
        return;
    }

    let content = lines_to_string(source, start_line, end_line);
    let symbol_ids = group.iter().map(|symbol| symbol.id.clone()).collect::<Vec<_>>();
    let symbol_id = symbol_ids.first().cloned();

    chunks.push(Chunk {
        id: format!("chk-{}-{}-{}", file_path.replace('/', "_"), start_line, end_line),
        file_path: file_path.to_string(),
        start_line,
        end_line,
        content: content.clone(),
        text: content.clone(),
        md5: blake3::hash(content.as_bytes()).to_hex().to_string(),
        size: content.len(),
        language: None,
        symbol_id,
        symbol_ids,
        chunk_kind: Some("Symbol".into()),
        metadata: None,
    });
}

fn symbol_lines(symbol: &Symbol) -> usize {
    symbol.end_line.saturating_sub(symbol.start_line) + 1
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
    fn groups_adjacent_symbols_until_limit() {
        let source = "fn a() {}\nfn b() {}\nfn c() {}\n";
        let chunker = SizeLimitedChunker::new(4);
        let chunks = chunker.chunk_file(
            "src/lib.rs",
            source,
            Some(&vec![symbol("sym::a", 1, 1), symbol("sym::b", 2, 2), symbol("sym::c", 3, 3)]),
        );

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol_ids, vec!["sym::a", "sym::b", "sym::c"]);
        assert!(chunks[0].content.contains("fn a() {}"));
        assert!(chunks[0].content.contains("fn c() {}"));
    }

    #[test]
    fn splits_between_symbols_when_limit_is_exceeded() {
        let source = "fn a() {}\nfn b() {}\nfn c() {}\n";
        let chunker = SizeLimitedChunker::new(2);
        let chunks = chunker.chunk_file(
            "src/lib.rs",
            source,
            Some(&vec![symbol("sym::a", 1, 1), symbol("sym::b", 2, 2), symbol("sym::c", 3, 3)]),
        );

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].symbol_ids, vec!["sym::a", "sym::b"]);
        assert_eq!(chunks[1].symbol_ids, vec!["sym::c"]);
        assert!(chunks[0].content.contains("fn a() {}"));
        assert!(chunks[1].content.contains("fn c() {}"));
    }

    #[test]
    fn keeps_oversized_symbol_in_its_own_chunk() {
        let source = "fn big() {\n    a();\n    b();\n    c();\n}\n";
        let chunker = SizeLimitedChunker::new(2);
        let chunks = chunker.chunk_file("src/lib.rs", source, Some(&vec![symbol("sym::big", 1, 5)]));

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].symbol_ids, vec!["sym::big"]);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 5);
    }

    #[test]
    fn falls_back_to_full_file_without_symbols() {
        let source = "fn main() {}\n";
        let chunker = SizeLimitedChunker::new(2);
        let chunks = chunker.chunk_file("src/lib.rs", source, None);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_kind.as_deref(), Some("FullFile"));
        assert!(chunks[0].content.contains("fn main"));
    }
}
