use rust_indexer::application::chunking::{
    ChunkStrategy, ContextInjectionChunker, LineLimitedChunker, OverlapChunker, SemanticChunker,
    SizeLimitedChunker, SymbolBoundaryChunker,
};
use rust_indexer::domain::types::Symbol;

#[test]
fn smoke_multi_strategy_consistent_chunking() {
    let source = r#"use std::collections::HashMap;

pub struct UserService {
    repo: Repo,
}

impl UserService {
    pub fn new() -> Self {
        Self { repo: Repo::new() }
    }

    pub fn add(&self, user: User) -> Result<(), Error> {
        self.repo.save(user)
    }
}

fn helper_function() -> usize {
    42
}

#[test]
fn test_user_service() {
    let service = UserService::new();
    assert!(service.add(User::default()).is_ok());
}
"#;

    let file_path = "src/services/user.rs";
    let symbols = vec![
        Symbol {
            id: "sym::UserService".into(),
            name: "UserService".into(),
            kind: "struct".into(),
            scope: None,
            file_path: file_path.into(),
            start_line: 4,
            end_line: 6,
            signature: None,
        },
        Symbol {
            id: "sym::UserService::impl".into(),
            name: "UserService".into(),
            kind: "impl".into(),
            scope: None,
            file_path: file_path.into(),
            start_line: 8,
            end_line: 18,
            signature: Some("impl UserService {".into()),
        },
        Symbol {
            id: "sym::UserService::new".into(),
            name: "new".into(),
            kind: "method".into(),
            scope: Some("UserService".into()),
            file_path: file_path.into(),
            start_line: 9,
            end_line: 11,
            signature: Some("pub fn new() -> Self".into()),
        },
        Symbol {
            id: "sym::UserService::add".into(),
            name: "add".into(),
            kind: "method".into(),
            scope: Some("UserService".into()),
            file_path: file_path.into(),
            start_line: 13,
            end_line: 15,
            signature: Some("pub fn add(&self, user: User) -> Result<(), Error>".into()),
        },
        Symbol {
            id: "sym::helper_function".into(),
            name: "helper_function".into(),
            kind: "function".into(),
            scope: None,
            file_path: file_path.into(),
            start_line: 20,
            end_line: 22,
            signature: Some("fn helper_function() -> usize".into()),
        },
        Symbol {
            id: "sym::test_user_service".into(),
            name: "test_user_service".into(),
            kind: "function".into(),
            scope: None,
            file_path: file_path.into(),
            start_line: 25,
            end_line: 30,
            signature: Some("#[test]\nfn test_user_service()".into()),
        },
    ];

    let symbol_chunker = SymbolBoundaryChunker::new(0);
    let symbol_chunks = symbol_chunker.chunk_file(file_path, source, Some(&symbols));
    assert_eq!(symbol_chunks.len(), symbols.len());
    assert!(symbol_chunks.iter().all(|c| c.symbol_ids.len() == 1));

    let semantic_chunks = SemanticChunker::new(0).chunk_file(file_path, source, Some(&symbols));
    assert_eq!(semantic_chunks.len(), 3);
    assert!(semantic_chunks.iter().any(|c| c.symbol_ids.iter().any(|id| id.contains("UserService"))));

    let size_chunks = SizeLimitedChunker::new(15).chunk_file(file_path, source, Some(&symbols));
    assert!(!size_chunks.is_empty());
    assert!(size_chunks.iter().all(|c| c.end_line >= c.start_line));
    assert!(size_chunks.iter().all(|c| (c.end_line - c.start_line + 1) <= 15));

    let line_chunks = LineLimitedChunker::new(20).chunk_file(file_path, source, Some(&symbols));
    assert!(!line_chunks.is_empty());
    assert!(line_chunks.iter().all(|c| (c.end_line - c.start_line + 1) <= 20));

    let context_chunks = ContextInjectionChunker::new(SemanticChunker::new(0))
        .chunk_file(file_path, source, Some(&symbols));
    assert!(!context_chunks.is_empty());
    assert!(context_chunks.iter().all(|c| c.metadata.as_ref().map(|m| m.get("has_context_prefix").is_some()).unwrap_or(false)));

    let overlap_chunks = OverlapChunker::new(SemanticChunker::new(0), 2)
        .chunk_file(file_path, source, Some(&symbols));
    assert!(!overlap_chunks.is_empty());
    assert!(overlap_chunks.iter().any(|c| c.metadata.as_ref().map(|m| m.get("previous_chunk_id").is_some() || m.get("next_chunk_id").is_some()).unwrap_or(false)));

    let pipeline_chunks = ContextInjectionChunker::new(OverlapChunker::new(SemanticChunker::new(30), 1))
        .chunk_file(file_path, source, Some(&symbols));
    assert!(!pipeline_chunks.is_empty());
    assert!(pipeline_chunks.iter().any(|c| c.metadata.is_some()));

    let no_symbol_chunks = symbol_chunker.chunk_file(file_path, source, None);
    assert_eq!(no_symbol_chunks.len(), 1);
    assert_eq!(no_symbol_chunks[0].content, source);

    let empty_symbols: Vec<Symbol> = vec![];
    let empty_chunks = symbol_chunker.chunk_file(file_path, source, Some(&empty_symbols));
    assert_eq!(empty_chunks.len(), 1);

    for chunk in symbol_chunks
        .iter()
        .chain(semantic_chunks.iter())
        .chain(size_chunks.iter())
        .chain(line_chunks.iter())
        .chain(context_chunks.iter())
        .chain(overlap_chunks.iter())
        .chain(pipeline_chunks.iter())
    {
        assert!(chunk.start_line >= 1);
        assert!(chunk.end_line >= chunk.start_line);
        assert!(!chunk.content.is_empty());
        assert!(!chunk.id.is_empty());
    }
}
