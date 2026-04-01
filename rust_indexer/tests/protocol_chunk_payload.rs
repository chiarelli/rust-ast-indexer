use rust_indexer::application::protocol::{ChunkEventPayload, ChunkKind};
use rust_indexer::domain::types::Chunk;

#[test]
fn build_and_serialize_chunk_payload_matches_expected() {
    let domain = Chunk {
        id: "chunk-2".into(),
        file_path: "src/main.rs".into(),
        start_line: 2,
        end_line: 4,
        text: "fn foo() {}\nfn bar() {}".into(),
        md5: "def456".into(),
        size: 34,
        language: Some("rust".into()),
        symbol_id: None,
        chunk_kind: Some("Symbol".into()),
    };

    let payload: ChunkEventPayload = domain.into();
    assert_eq!(payload.chunk_id, "chunk-2");
    assert_eq!(payload.file, "src/main.rs");
    assert_eq!(payload.language.as_deref(), Some("rust"));
    assert_eq!(payload.symbol_id, None);
    assert_eq!(payload.start_line, 2);
    assert_eq!(payload.end_line, 4);
    assert_eq!(payload.chunk_md5, "def456");
    assert_eq!(payload.size, 34);
    assert_eq!(payload.chunk_kind, ChunkKind::Symbol);

    // Serialize to JSON and ensure keys exist
    let val = serde_json::to_value(&payload).unwrap();
    assert!(val.get("chunk_id").is_some());
    assert!(val.get("chunk_kind").is_some());
    assert!(val.get("file").is_some());
}
