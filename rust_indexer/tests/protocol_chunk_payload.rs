use rust_indexer::application::protocol::{ChunkEventPayload, ChunkKind};
use rust_indexer::domain::types::Chunk;

#[test]
fn build_and_serialize_chunk_payload_matches_expected() {
    let domain = Chunk {
        id: "chunk-2".into(),
        file_path: "src/main.rs".into(),
        start_line: 2,
        end_line: 4,
        content: "fn foo() {}\nfn bar() {}".into(),
        text: "fn foo() {}\nfn bar() {}".into(),
        md5: "def456".into(),
        size: 34,
        language: Some("rust".into()),
        symbol_id: None,
        symbol_ids: vec![],
        chunk_kind: Some("Symbol".into()),
        metadata: None,
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

/// Regressão (ALTO): `symbol_ids` (plural) deve chegar ao fio com os nomes CRUS
/// deduplicados — sem ele, os símbolos desambiguados por `dedupe_symbol_ids`
/// sumiriam do payload e o consumidor nunca veria `handle@7`.
#[test]
fn chunk_payload_emits_all_symbol_ids_without_file_prefix() {
    let domain = Chunk {
        id: "chunk-3".into(),
        file_path: "src/app.ts".into(),
        start_line: 1,
        end_line: 20,
        content: "...".into(),
        text: "...".into(),
        md5: "abc".into(),
        size: 3,
        language: Some("typescript".into()),
        symbol_id: Some("src/app.ts:handle".into()),
        symbol_ids: vec![
            "src/app.ts:handle".into(),
            "src/app.ts:handle@7".into(),
        ],
        chunk_kind: Some("Symbol".into()),
        metadata: None,
    };

    let payload: ChunkEventPayload = domain.into();
    assert_eq!(payload.symbol_id.as_deref(), Some("handle"));
    assert_eq!(payload.symbol_ids, vec!["handle", "handle@7"]);

    let val = serde_json::to_value(&payload).unwrap();
    assert_eq!(
        val["symbol_ids"],
        serde_json::json!(["handle", "handle@7"])
    );
}
