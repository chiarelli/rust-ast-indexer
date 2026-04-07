use crate::domain::types::{Chunk, Symbol};
use md5;

pub struct SymbolBoundaryChunker {
    pub max_lines: usize,
}

impl SymbolBoundaryChunker {
    pub fn new(max_lines: usize) -> Self {
        Self { max_lines }
    }
}

impl crate::application::chunking::ChunkStrategy for SymbolBoundaryChunker {
    fn chunk_file(&self, file_path: &str, source: &str, symbols: Option<&Vec<Symbol>>) -> Vec<Chunk> {
        // If no symbols provided, fall back to full file chunk
        if symbols.is_none() || symbols.as_ref().unwrap().is_empty() {
            let text = source.to_string();
            return vec![Chunk {
                id: format!("chk-{}", format!("{:x}", md5::compute(&text))),
                file_path: file_path.to_string(),
                start_line: 1,
                end_line: source.lines().count().max(1),
                content: text.clone(),
                text: text.clone(),
                md5: format!("{:x}", md5::compute(&text)),
                size: source.len(),
                language: None,
                symbol_id: None,
                symbol_ids: vec![],
                chunk_kind: Some("FullFile".into()),
                metadata: None,
            }];
        }

        let syms = symbols.unwrap();
        let mut chunks = Vec::new();

        for s in syms.iter() {
            let start = s.start_line;
            let end = s.end_line;
            let content = sourcelines_to_string(source, start, end);
            let text = content.clone();
            let chunk = Chunk {
                id: format!("chk-{}-{}-{}", file_path.replace('/', "_"), start, end),
                file_path: file_path.to_string(),
                start_line: start,
                end_line: end,
                content,
                text: text.clone(),
                md5: format!("{:x}", md5::compute(&text)),
                size: (end - start + 1),
                language: None,
                symbol_id: Some(s.id.clone()),
                symbol_ids: vec![s.id.clone()],
                chunk_kind: Some("Symbol".into()),
                metadata: None,
            };
            chunks.push(chunk);
        }

        // Optionally enforce max_lines by merging/splitting - simple approach: drop oversized chunks
        if self.max_lines > 0 {
            chunks.retain(|c| (c.end_line - c.start_line + 1) <= self.max_lines);
        }

        chunks
    }
}

fn sourcelines_to_string(source: &str, start: usize, end: usize) -> String {
    let mut out = String::new();
    for (i, line) in source.lines().enumerate() {
        let idx = i + 1;
        if idx >= start && idx <= end {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}
