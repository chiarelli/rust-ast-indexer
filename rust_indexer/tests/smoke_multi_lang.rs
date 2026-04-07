use std::sync::Arc;
use tempfile::TempDir;
use rust_indexer::{
    application::indexer::{Indexer, IndexOptions},
    infra::parser_pool::ParserPool,
    app::bootstrap::{ApplicationContext, Registry},
    adapters,
};
use std::path::{Path, PathBuf};

fn build_ctx() -> Arc<ApplicationContext> {
    let registry = Arc::new(Registry::new());
    registry.register("rust", Arc::new(adapters::rust::RustAdapter));
    registry.register("typescript", Arc::new(adapters::typescript::TypeScriptAdapter));
    registry.register("javascript", Arc::new(adapters::typescript::TypeScriptAdapter));
    registry.register("java", Arc::new(adapters::java::JavaAdapter));
    
    let pool = ParserPool::new();
    pool.register("rust", Arc::new(adapters::rust::RustAdapter));
    pool.register("typescript", Arc::new(adapters::typescript::TypeScriptAdapter));
    pool.register("javascript", Arc::new(adapters::typescript::TypeScriptAdapter));
    pool.register("java", Arc::new(adapters::java::JavaAdapter));

    let config = rust_indexer::app::bootstrap::Config::new(1, 10);
    Arc::new(ApplicationContext {
        registry,
        parser_pool: Arc::new(pool),
        config,
        metrics: None,
        logger: None,
    })
}

#[allow(dead_code)]
fn create_multi_lang_repo(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    files.push(create_rust_file(dir));
    files.push(create_typescript_file(dir));
    files.push(create_java_file(dir));

    files
}

fn create_rust_file(dir: &Path) -> PathBuf {
    let path = dir.join("src/main.rs");
    let content = "pub fn add(a: i32, b: i32) -> i32 { a + b }\npub mod utils {\n    pub struct Helper { value: u32 }\n}\n";
    std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir src");
    std::fs::write(&path, content).expect("write rust");
    path
}

fn create_typescript_file(dir: &Path) -> PathBuf {
    let path = dir.join("lib/service.ts");
    let content = "function process(data) { return data; }\nclass UserService { add(user) { return user; } }\n";
    std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir lib");
    std::fs::write(&path, content).expect("write ts");
    path
}

fn create_java_file(dir: &Path) -> PathBuf {
    let path = dir.join("com/example/Repository.java");
    let content = "public class Repository {\n    public void save(Object entity) {}\n    private String url;\n}\n";
    std::fs::create_dir_all(path.parent().unwrap()).expect("mkdir com/example");
    std::fs::write(&path, content).expect("write java");
    path
}

// --- Happy path tests ---

#[test]
fn smoke_multi_lang_produces_symbols_for_all_languages() {
    let td = TempDir::new().expect("tempdir");
    create_multi_lang_repo(td.path());

    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let opts = IndexOptions {
        max_concurrency: 1,
        explicit_files: None,
        extract_imports: false,
        extract_calls: false,
    };
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), opts, None).expect("index should succeed");

    assert_eq!(res.files.len(), 3, "should list 3 multi-lang files");
    assert!(!res.chunks.is_empty(), "should produce at least one chunk");

    let rust_files: Vec<_> = res.files.iter().filter(|f| f.path.ends_with(".rs")).collect();
    let ts_files: Vec<_> = res.files.iter().filter(|f| f.path.ends_with(".ts")).collect();
    let java_files: Vec<_> = res.files.iter().filter(|f| f.path.ends_with(".java")).collect();

    assert_eq!(rust_files.len(), 1);
    assert_eq!(ts_files.len(), 1);
    assert_eq!(java_files.len(), 1);
}

#[test]
fn smoke_multi_lang_rust_symbols_extracted() {
    let td = TempDir::new().expect("tempdir");
    create_multi_lang_repo(td.path());

    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), IndexOptions { max_concurrency: 1, explicit_files: None, extract_imports: false, extract_calls: false }, None).expect("index");

    let chunks: Vec<_> = res.chunks.iter().filter(|c| c.text.contains("fn add")).collect();
    assert!(!chunks.is_empty(), "should find a chunk with fn add");
}

#[test]
fn smoke_multi_lang_ts_symbols_extracted() {
    let td = TempDir::new().expect("tempdir");
    create_multi_lang_repo(td.path());

    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), IndexOptions { max_concurrency: 1, explicit_files: None, extract_imports: false, extract_calls: false }, None).expect("index");

    let chunks: Vec<_> = res.chunks.iter().filter(|c| c.text.contains("function process") || c.text.contains("class UserService")).collect();
    assert!(!chunks.is_empty(), "should find a chunk with TS symbols");
}

#[test]
fn smoke_multi_lang_java_symbols_extracted() {
    let td = TempDir::new().expect("tempdir");
    create_multi_lang_repo(td.path());

    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), IndexOptions { max_concurrency: 1, explicit_files: None, extract_imports: false, extract_calls: false }, None).expect("index");

    let chunks: Vec<_> = res.chunks.iter().filter(|c| c.text.contains("class Repository")).collect();
    assert!(!chunks.is_empty(), "should find a chunk with class Repository");
}

#[test]
fn smoke_no_files_produces_empty_results() {
    let td = TempDir::new().expect("tempdir");
    
    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), IndexOptions { max_concurrency: 1, explicit_files: None, extract_imports: false, extract_calls: false }, None).expect("index");

    assert!(res.files.is_empty());
    assert!(res.chunks.is_empty());
}

#[test]
fn smoke_only_unsupported_files_skipped() {
    let td = TempDir::new().expect("tempdir");
    std::fs::write(td.path().join("README.md"), "# My Project\n").expect("write");
    std::fs::write(td.path().join("notes.txt"), "some notes\n").expect("write");
    std::fs::write(td.path().join("config.json"), "{}\n").expect("write");

    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), IndexOptions { max_concurrency: 1, explicit_files: None, extract_imports: false, extract_calls: false }, None).expect("index");

    // Walker returns all text files, but unsupported languages have no chunk language
    let supported: Vec<_> = res.chunks.iter().filter(|c| c.language.is_some()).collect();
    assert!(supported.is_empty(), "no supported-language chunks for unsupported files");
}

#[test]
fn smoke_mixed_supported_unsupported_only_processes_supported() {
    let td = TempDir::new().expect("tempdir");
    std::fs::write(td.path().join("main.rs"), "fn foo() {}\n").expect("write rust");
    std::fs::write(td.path().join("helper.py"), "def foo(): pass\n").expect("write py");
    std::fs::write(td.path().join("lib.rs"), "pub fn bar() {}\n").expect("write rust");

    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), IndexOptions { max_concurrency: 1, explicit_files: None, extract_imports: false, extract_calls: false }, None).expect("index");

    assert_eq!(res.files.len(), 3, "3 files listed (including .py)");
    let supported_chunks: Vec<_> = res.chunks.iter().filter(|c| c.language.as_deref() == Some("rust")).collect();
    assert_eq!(supported_chunks.len(), 2, "only .rs chunks produced");
}

#[test]
fn smoke_empty_rust_file_parses_without_error() {
    let td = TempDir::new().expect("tempdir");
    std::fs::write(td.path().join("empty.rs"), "").expect("write empty");

    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), IndexOptions { max_concurrency: 1, explicit_files: None, extract_imports: false, extract_calls: false }, None).expect("index");

    assert_eq!(res.files.len(), 1);
}

#[test]
fn smoke_whitespace_only_rust_file_parses_without_error() {
    let td = TempDir::new().expect("tempdir");
    std::fs::write(td.path().join("ws.rs"), "   \n\n   ").expect("write ws");

    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), IndexOptions { max_concurrency: 1, explicit_files: None, extract_imports: false, extract_calls: false }, None).expect("index");

    assert_eq!(res.files.len(), 1);
}

#[test]
fn smoke_deep_nested_directory_structure() {
    let td = TempDir::new().expect("tempdir");
    let deep_dir = td.path().join("a/b/c/d/e/f/g/h/i/j");
    std::fs::create_dir_all(&deep_dir).expect("create deep dirs");
    std::fs::write(deep_dir.join("deep.rs"), "fn deep() {}\n").expect("write deep");

    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), IndexOptions { max_concurrency: 1, explicit_files: None, extract_imports: false, extract_calls: false }, None).expect("index");

    assert_eq!(res.files.len(), 1);
    assert!(res.files[0].path.ends_with("j/deep.rs"));
}

#[test]
fn smoke_multiple_files_same_directory() {
    let td = TempDir::new().expect("tempdir");
    for i in 0..5 {
        std::fs::write(
            td.path().join(format!("file_{}.rs", i)),
            format!("fn func_{}() {{}}\n", i)
        ).expect("write");
    }

    let ctx = build_ctx();
    let indexer = Indexer::from_context(Arc::clone(&ctx));
    let res = indexer.index_path_parallel(td.path().to_str().unwrap(), IndexOptions { max_concurrency: 1, explicit_files: None, extract_imports: false, extract_calls: false }, None).expect("index");

    assert_eq!(res.files.len(), 5);
    assert_eq!(res.chunks.len(), 5);
}
