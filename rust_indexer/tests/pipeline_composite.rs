use rust_indexer::application::chunking::{ChunkStrategy, ContextInjectionChunker, OverlapChunker, SemanticChunker};
use rust_indexer::domain::types::Symbol;

#[allow(clippy::too_many_arguments)]
fn symbol(
    id: &str,
    name: &str,
    kind: &str,
    scope: Option<&str>,
    file_path: &str,
    start_line: usize,
    end_line: usize,
    signature: Option<&str>,
) -> Symbol {
    Symbol {
        id: id.into(),
        name: name.into(),
        kind: kind.into(),
        scope: scope.map(|value| value.into()),
        file_path: file_path.into(),
        start_line,
        end_line,
        signature: signature.map(|value| value.into()),
    }
}

#[test]
fn semantic_overlap_pipeline_expands_neighboring_chunks() {
    let file_path = "src/services/user.rs";
    let source = "use std::collections::HashMap;\n\npub struct UserService {\n    repo: Repo,\n}\n\nimpl UserService {\n    pub fn new() -> Self {\n        Self { repo: Repo::new() }\n    }\n\n    pub fn add(&self, user: User) -> Result<(), Error> {\n        self.repo.save(user)\n    }\n}\n\nfn helper_function() -> usize {\n    42\n}\n";
    let symbols = vec![
        symbol("sym::UserService", "UserService", "struct", None, file_path, 3, 5, None),
        symbol(
            "sym::UserService::impl",
            "UserService",
            "impl",
            None,
            file_path,
            7,
            14,
            Some("impl UserService {"),
        ),
        symbol("sym::UserService::new", "new", "method", Some("UserService"), file_path, 8, 10, Some("pub fn new() -> Self")),
        symbol("sym::UserService::add", "add", "method", Some("UserService"), file_path, 12, 14, Some("pub fn add(&self, user: User) -> Result<(), Error>")),
        symbol("sym::helper_function", "helper_function", "function", None, file_path, 16, 18, Some("fn helper_function() -> usize")),
    ];

    let pipeline = OverlapChunker::new(SemanticChunker::new(0), 1);
    let chunks = pipeline.chunk_file(file_path, source, Some(&symbols));

    assert!(chunks.len() >= 2);
    assert!(chunks.iter().any(|chunk| chunk.metadata.as_ref().and_then(|meta| meta.get("previous_chunk_id")).is_some() || chunk.metadata.as_ref().and_then(|meta| meta.get("next_chunk_id")).is_some()));
    assert!(chunks.iter().any(|chunk| chunk.text.contains("UserService")));
}

#[test]
fn semantic_context_pipeline_injects_imports_and_scopes() {
    let file_path = "src/services/user.rs";
    let source = "use std::collections::HashMap;\nuse std::fmt;\n\npub struct UserService {\n    repo: Repo,\n}\n\nimpl UserService {\n    pub fn new() -> Self {\n        Self { repo: Repo::new() }\n    }\n}\n";
    let symbols = vec![
        symbol("sym::UserService", "UserService", "struct", None, file_path, 4, 6, None),
        symbol(
            "sym::UserService::impl",
            "UserService",
            "impl",
            None,
            file_path,
            8,
            11,
            Some("impl UserService {"),
        ),
        symbol("sym::UserService::new", "new", "method", Some("UserService"), file_path, 9, 11, Some("pub fn new() -> Self")),
    ];

    let pipeline = ContextInjectionChunker::new(SemanticChunker::new(0));
    let chunks = pipeline.chunk_file(file_path, source, Some(&symbols));

    assert!(!chunks.is_empty());
    assert!(chunks.iter().all(|chunk| chunk.content.starts_with("use std::collections::HashMap;\nuse std::fmt;\n\n")));
    assert!(chunks.iter().any(|chunk| chunk.content.contains("scope: UserService")));
    assert!(chunks.iter().all(|chunk| chunk.metadata.as_ref().and_then(|meta| meta.get("has_context_prefix")).and_then(|value| value.as_bool()) == Some(true)));
}

#[test]
fn composed_pipeline_preserves_overlap_and_context_metadata() {
    let file_path = "src/services/logger.rs";
    let source = "use std::io;\n\npub struct Logger;\n\nimpl Logger {\n    pub fn new() -> Self {\n        Self\n    }\n\n    pub fn log(&self, message: &str) {\n        println!(\"{}\", message);\n    }\n}\n\nfn helper() {\n    println!(\"helper\");\n}\n";
    let symbols = vec![
        symbol("sym::Logger", "Logger", "struct", None, file_path, 3, 3, None),
        symbol(
            "sym::Logger::impl",
            "Logger",
            "impl",
            None,
            file_path,
            5,
            13,
            Some("impl Logger {"),
        ),
        symbol("sym::Logger::new", "new", "method", Some("Logger"), file_path, 6, 8, Some("pub fn new() -> Self")),
        symbol("sym::Logger::log", "log", "method", Some("Logger"), file_path, 10, 12, Some("pub fn log(&self, message: &str)")),
        symbol("sym::helper", "helper", "function", None, file_path, 15, 17, Some("fn helper()")),
    ];

    let pipeline = ContextInjectionChunker::new(OverlapChunker::new(SemanticChunker::new(0), 2));
    let chunks = pipeline.chunk_file(file_path, source, Some(&symbols));

    assert!(chunks.iter().any(|chunk| chunk.metadata.as_ref().and_then(|meta| meta.get("has_context_prefix")).and_then(|value| value.as_bool()) == Some(true)));
    assert!(chunks.iter().any(|chunk| chunk.metadata.as_ref().and_then(|meta| meta.get("overlap_lines")).is_some()));
    assert!(chunks.iter().any(|chunk| chunk.text.contains("scope: Logger")));
}
