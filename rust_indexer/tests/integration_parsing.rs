use tempfile::tempdir;

#[test]
fn integration_parser_handles_rust_file() {
    // Ensure binary exists via cargo build and then run the library via tests (we'll run indexer functions directly)
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, b"fn main() { println!(\"hello\"); }\n").unwrap();

    // Call indexer::index_path_parallel via library interface in a small harness
    // Using rust_indexer as a library is easier in tests; call application::indexer::Indexer::index_path_parallel
    let indexer = rust_indexer::application::indexer::Indexer::new();
    let opts = rust_indexer::application::indexer::IndexOptions { max_concurrency: 2, explicit_files: None, extract_imports: false, extract_calls: false };
    let res = indexer.index_path_parallel(dir.path().to_str().unwrap(), opts, None).unwrap();

    assert_eq!(res.files.len(), 1);
    assert_eq!(res.chunks.len(), 1);
    let chunk = &res.chunks[0];
    assert!(chunk.text.contains("fn main"));
}
