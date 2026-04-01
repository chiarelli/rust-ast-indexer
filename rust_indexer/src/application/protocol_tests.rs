#[cfg(test)]
mod tests {
    use crate::application::protocol::{ChunkEventPayload, ChunkKind};
    use crate::domain::types::Chunk;

    #[test]
    fn chunk_from_domain_chunk_maps_fields_correctly() {
        let domain = Chunk {
            id: "chunk-1".into(),
            file_path: "src/lib.rs".into(),
            start_line: 1,
            end_line: 10,
            text: "fn main() {}".into(),
            md5: "abc123".into(),
            size: 12,
            language: Some("rust".into()),
            symbol_id: Some("sym-1".into()),
            chunk_kind: Some("FullFile".into()),
        };

        let payload: ChunkEventPayload = domain.into();
        assert_eq!(payload.chunk_id, "chunk-1");
        assert_eq!(payload.file, "src/lib.rs");
        assert_eq!(payload.language.as_deref(), Some("rust"));
        assert_eq!(payload.symbol_id.as_deref(), Some("sym-1"));
        assert_eq!(payload.start_line, 1);
        assert_eq!(payload.end_line, 10);
        assert_eq!(payload.chunk_md5, "abc123");
        assert_eq!(payload.size, 12);
        assert_eq!(payload.chunk_kind, ChunkKind::FullFile);
    }
}
