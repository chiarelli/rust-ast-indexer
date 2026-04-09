use crate::application::chunking::ChunkStrategy;
use crate::domain::types::{Chunk, Symbol};

pub struct OverlapChunker<S> {
    inner: S,
    overlap_lines: usize,
}

impl<S> OverlapChunker<S> {
    pub fn new(inner: S, overlap_lines: usize) -> Self {
        Self { inner, overlap_lines }
    }
}

impl<S: ChunkStrategy> ChunkStrategy for OverlapChunker<S> {
    fn chunk_file(&self, file_path: &str, source: &str, symbols: Option<&Vec<Symbol>>) -> Vec<Chunk> {
        let chunks = self.inner.chunk_file(file_path, source, symbols);
        if self.overlap_lines == 0 || chunks.len() < 2 {
            return chunks;
        }

        let mut reordered = chunks
            .into_iter()
            .enumerate()
            .collect::<Vec<_>>();
        reordered.sort_by_key(|(idx, chunk)| (chunk.file_path.clone(), chunk.start_line, chunk.end_line, *idx));

        let mut output = Vec::with_capacity(reordered.len());
        let mut start = 0usize;
        while start < reordered.len() {
            let file_path_key = reordered[start].1.file_path.clone();
            let mut end = start + 1;
            while end < reordered.len() && reordered[end].1.file_path == file_path_key {
                end += 1;
            }

            let group = reordered[start..end].to_vec();
            for (index, (_, chunk)) in group.into_iter().enumerate() {
                let file_chunks_len = end - start;
                let has_prev = index > 0;
                let has_next = index + 1 < file_chunks_len;
                let expanded_start = if has_prev {
                    chunk.start_line.saturating_sub(self.overlap_lines)
                } else {
                    chunk.start_line
                };
                let expanded_end = if has_next {
                    chunk.end_line.saturating_add(self.overlap_lines)
                } else {
                    chunk.end_line
                };
                output.push(expand_chunk(chunk, source, expanded_start, expanded_end, self.overlap_lines));
            }

            start = end;
        }

        output
    }
}

fn expand_chunk(mut chunk: Chunk, source: &str, start_line: usize, end_line: usize, overlap_lines: usize) -> Chunk {
    let content = lines_to_string(source, start_line, end_line);
    let digest = blake3::hash(content.as_bytes()).to_hex().to_string();

    let mut metadata = chunk.metadata.take().unwrap_or_default();
    metadata.insert(
        "overlap_lines".to_string(),
        serde_json::Value::Number(serde_json::Number::from(overlap_lines as u64)),
    );
    if !metadata.contains_key("chunk_strategy") {
        metadata.insert(
            "chunk_strategy".to_string(),
            serde_json::Value::String("overlap".to_string()),
        );
    }

    chunk.id = format!("chk-ov-{}", digest);
    chunk.start_line = start_line;
    chunk.end_line = end_line;
    chunk.content = content.clone();
    chunk.text = content;
    chunk.md5 = digest;
    chunk.size = chunk.content.len();
    chunk.metadata = Some(metadata);
    chunk
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
    use std::collections::HashMap;

    use super::*;
    use crate::application::chunking::ChunkStrategy;

    struct StubChunker;

    impl ChunkStrategy for StubChunker {
        fn chunk_file(&self, file_path: &str, source: &str, _symbols: Option<&Vec<Symbol>>) -> Vec<Chunk> {
            vec![
                Chunk {
                    id: format!("chk-{}-1", file_path),
                    file_path: file_path.into(),
                    start_line: 1,
                    end_line: 2,
                    content: "fn a() {}\n\n".into(),
                    text: "fn a() {}\n\n".into(),
                    md5: "a".into(),
                    size: 11,
                    language: Some("rust".into()),
                    symbol_id: Some("sym::a".into()),
                    symbol_ids: vec!["sym::a".into()],
                    chunk_kind: Some("Symbol".into()),
                    metadata: Some(HashMap::from([(String::from("source"), serde_json::Value::String(source.len().to_string()))])),
                },
                Chunk {
                    id: format!("chk-{}-2", file_path),
                    file_path: file_path.into(),
                    start_line: 4,
                    end_line: 5,
                    content: "fn b() {}\n\n".into(),
                    text: "fn b() {}\n\n".into(),
                    md5: "b".into(),
                    size: 11,
                    language: Some("rust".into()),
                    symbol_id: Some("sym::b".into()),
                    symbol_ids: vec!["sym::b".into()],
                    chunk_kind: Some("Symbol".into()),
                    metadata: None,
                },
            ]
        }
    }

    #[test]
    fn adds_neighbor_overlap_to_chunks() {
        let source = "fn a() {}\nline1\nline2\nfn b() {}\nline4\n";
        let chunker = OverlapChunker::new(StubChunker, 1);
        let chunks = chunker.chunk_file("src/lib.rs", source, None);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 3);
        assert!(chunks[0].content.contains("line1"));
        assert_eq!(chunks[1].start_line, 3);
        assert_eq!(chunks[1].end_line, 5);
        assert!(chunks[1].content.contains("line2"));
        assert_eq!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("overlap_lines")).and_then(|value| value.as_u64()), Some(1));
        assert_eq!(chunks[0].metadata.as_ref().and_then(|meta| meta.get("chunk_strategy")), Some(&serde_json::Value::String("overlap".to_string())));
    }

    #[test]
    fn no_overlap_is_noop() {
        let source = "fn a() {}\nfn b() {}\n";
        let chunker = OverlapChunker::new(StubChunker, 0);
        let chunks = chunker.chunk_file("src/lib.rs", source, None);

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].start_line, 1);
        assert_eq!(chunks[0].end_line, 2);
        assert_eq!(chunks[1].start_line, 4);
        assert_eq!(chunks[1].end_line, 5);
    }
}
