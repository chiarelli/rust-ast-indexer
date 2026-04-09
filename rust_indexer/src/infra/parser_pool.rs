// ParserPool using LanguageAdapter instances per language
// Thread-safe pool providing adapters for parsing

use std::sync::Arc;
use crate::adapters::LanguageAdapter;
use dashmap::DashMap;

pub struct ParserPool {
    adapters: DashMap<String, Arc<dyn LanguageAdapter>>,
}

impl Default for ParserPool {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserPool {
    pub fn new() -> Self {
        Self {
            adapters: DashMap::new(),
        }
    }

    /// Register a language adapter into the pool
    pub fn register(&self, lang: &str, adapter: Arc<dyn LanguageAdapter>) {
        self.adapters.insert(lang.to_string(), adapter);
    }

    /// Get an adapter for the given language
    pub fn get(&self, lang: &str) -> Option<Arc<dyn LanguageAdapter>> {
        self.adapters.get(lang).map(|e| Arc::clone(e.value()))
    }

    /// List supported languages
    pub fn languages(&self) -> Vec<String> {
        self.adapters.iter().map(|e| e.key().clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::parser::ParsedFile;
    use crate::domain::types::Symbol;
    use anyhow::Result;

    struct MockAdapter;
    impl LanguageAdapter for MockAdapter {
        fn parse_source(&self, source: &str) -> Result<ParsedFile> {
            Ok(ParsedFile { language: "mock".to_string(), source_len: source.len(), source: source.to_string() })
        }
        fn extract_symbols(&self, _parsed: &ParsedFile) -> Result<Vec<Symbol>> {
            Ok(vec![])
        }
        fn box_clone(&self) -> Box<dyn LanguageAdapter> { Box::new(MockAdapter) }
    }

    #[test]
    fn pool_register_and_get() {
        let pool = ParserPool::new();
        pool.register("test", Arc::new(MockAdapter));
        let adapter = pool.get("test");
        assert!(adapter.is_some());
        let parsed = adapter.unwrap().parse_source("fn test() {}").unwrap();
        assert_eq!(parsed.language, "mock");
    }

    #[test]
    fn pool_get_missing_lang_returns_none() {
        let pool = ParserPool::new();
        assert!(pool.get("nonexistent").is_none());
    }

    #[test]
    fn pool_languages_lists_registered() {
        let pool = ParserPool::new();
        pool.register("rust", Arc::new(MockAdapter));
        pool.register("java", Arc::new(MockAdapter));
        let mut langs = pool.languages();
        langs.sort();
        assert_eq!(langs, vec!["java", "rust"]);
    }
}

#[cfg(all(test, feature = "parsing"))]
mod integration_tests {
    use super::*;
    use crate::adapters::rust::RustAdapter;
    use crate::adapters::typescript::TypeScriptAdapter;
    use crate::adapters::java::JavaAdapter;

    fn build_pool() -> ParserPool {
        let pool = ParserPool::new();
        pool.register("rust", Arc::new(RustAdapter));
        pool.register("typescript", Arc::new(TypeScriptAdapter));
        pool.register("javascript", Arc::new(TypeScriptAdapter));
        pool.register("java", Arc::new(JavaAdapter));
        pool
    }

    #[test]
    fn pool_parse_and_extract_rust() {
        let pool = build_pool();
        let adapter = pool.get("rust").expect("rust adapter should be registered");

        let source = r#"
pub struct Config {
    pub host: String,
    pub port: u16,
}

pub fn start(config: Config) {
    println!("starting on {}:{}", config.host, config.port);
}

trait Repository {
    fn find(&self, id: u32) -> Option<String>;
}
"#;
        let parsed = adapter.parse_source(source).expect("parse should succeed");
        assert_eq!(parsed.language, "rust");

        let symbols = adapter.extract_symbols(&parsed).expect("extract should succeed");
        assert!(symbols.iter().any(|s| s.kind == "struct" && s.name == "Config"));
        assert!(symbols.iter().any(|s| s.kind == "function" && s.name == "start"));
        assert!(symbols.iter().any(|s| s.kind == "trait" && s.name == "Repository"));
    }

    #[test]
    fn pool_parse_and_extract_typescript() {
        let pool = build_pool();
        let adapter = pool.get("typescript").expect("typescript adapter should be registered");

        let source = r#"
class UserService {
    private users = [];

    addUser(name) {
        this.users.push(name);
    }
}

function getVersion() {
    return "1.0.0";
}
"#;
        let parsed = adapter.parse_source(source).expect("parse should succeed");
        assert_eq!(parsed.language, "typescript");

        let symbols = adapter.extract_symbols(&parsed).expect("extract should succeed");
        assert!(symbols.iter().any(|s| s.kind == "class" && s.name == "UserService"), 
                "Expected class UserService, got: {:?}", 
                symbols.iter().map(|s| format!("{}:{}", s.kind, s.name)).collect::<Vec<_>>());
        assert!(symbols.iter().any(|s| s.kind == "function" && s.name == "getVersion"),
                "Expected function getVersion, got: {:?}",
                symbols.iter().map(|s| format!("{}:{}", s.kind, s.name)).collect::<Vec<_>>());
    }

    #[test]
    fn pool_parse_and_extract_java() {
        let pool = build_pool();
        let adapter = pool.get("java").expect("java adapter should be registered");

        let source = r#"
import java.util.List;
import java.util.ArrayList;

public class UserRepository {
    private List<String> users;

    public UserRepository() {
        this.users = new ArrayList<>();
    }

    public void add(String user) {
        this.users.add(user);
    }

    public List<String> findAll() {
        return this.users;
    }
}
"#;
        let parsed = adapter.parse_source(source).expect("parse should succeed");
        assert_eq!(parsed.language, "java");

        let symbols = adapter.extract_symbols(&parsed).expect("extract should succeed");
        assert!(symbols.iter().any(|s| s.kind == "class" && s.name == "UserRepository"));
        assert!(symbols.iter().any(|s| s.kind == "constructor" && s.name == "UserRepository"));
        assert!(symbols.iter().any(|s| s.kind == "method" && s.name == "add"));
        assert!(symbols.iter().any(|s| s.kind == "field" && s.name == "users"));
    }

    #[test]
    fn pool_multi_language_support() {
        let pool = build_pool();
        let langs = pool.languages();

        assert!(langs.contains(&"rust".to_string()));
        assert!(langs.contains(&"typescript".to_string()));
        assert!(langs.contains(&"javascript".to_string()));
        assert!(langs.contains(&"java".to_string()));
        assert_eq!(langs.len(), 4);
    }

    #[test]
    fn pool_parse_and_extract_javascript_alias() {
        let pool = build_pool();
        let adapter = pool.get("javascript").expect("javascript adapter should be registered");

        let source = r#"
function greet(name) {
    return `Hello, ${name}!`;
}

const VERSION = "1.0.0";
"#;
        let parsed = adapter.parse_source(source).expect("parse should succeed");
        assert_eq!(parsed.language, "typescript");

        let symbols = adapter.extract_symbols(&parsed).expect("extract should succeed");
        assert!(symbols.iter().any(|s| s.kind == "function" && s.name == "greet"));
        assert!(symbols.iter().any(|s| s.kind == "variable" && s.name == "VERSION"));
    }

    #[test]
    fn pool_extract_nested_symbols_rust() {
        let pool = build_pool();
        let adapter = pool.get("rust").expect("rust adapter should be registered");

        let source = r#"
mod api {
    pub struct Handler { name: String }

    impl Handler {
        pub fn handle(&self, req: &str) -> String {
            format!("handled: {}", req)
        }
    }
}
"#;
        let parsed = adapter.parse_source(source).expect("parse should succeed");
        let symbols = adapter.extract_symbols(&parsed).expect("extract should succeed");

        let kinds: Vec<&str> = symbols.iter().map(|s| s.kind.as_str()).collect();
        assert!(kinds.contains(&"mod"), "expected 'mod', got: {:?}", kinds);
        assert!(kinds.contains(&"struct"), "expected 'struct', got: {:?}", kinds);
        assert!(kinds.contains(&"impl"), "expected 'impl', got: {:?}", kinds);
        assert!(kinds.contains(&"function"), "expected 'function', got: {:?}", kinds);
    }

    #[test]
    fn pool_extract_nested_symbols_typescript() {
        let pool = build_pool();
        let adapter = pool.get("typescript").expect("typescript adapter should be registered");

        let source = r#"
class Database {
    connect() { return true; }

    async query(sql) {
        return [];
    }
}

export const db = new Database();
"#;
        let parsed = adapter.parse_source(source).expect("parse should succeed");
        let symbols = adapter.extract_symbols(&parsed).expect("extract should succeed");

        assert!(symbols.iter().any(|s| s.kind == "class" && s.name == "Database"));
        assert!(symbols.iter().any(|s| s.kind == "method" && s.name == "connect"));
    }

    #[test]
    fn pool_extract_nested_symbols_java() {
        let pool = build_pool();
        let adapter = pool.get("java").expect("java adapter should be registered");

        let source = r#"
public class OrderService {
    private OrderRepository repository;

    public OrderService(OrderRepository repository) {
        this.repository = repository;
    }

    public Order findById(Long id) {
        return repository.findById(id);
    }
}
"#;
        let parsed = adapter.parse_source(source).expect("parse should succeed");
        let symbols = adapter.extract_symbols(&parsed).expect("extract should succeed");

        assert!(symbols.iter().any(|s| s.kind == "class" && s.name == "OrderService"));
        assert!(symbols.iter().any(|s| s.kind == "constructor" && s.name == "OrderService"));
        assert!(symbols.iter().any(|s| s.kind == "method" && s.name == "findById"));
        assert!(symbols.iter().any(|s| s.kind == "field" && s.name == "repository"));
    }

    #[test]
    fn pool_empty_source_all_languages() {
        let pool = build_pool();
        for lang in &["rust", "typescript", "java"] {
            let adapter = pool.get(lang).unwrap_or_else(|| panic!("{} adapter should exist", lang));
            let parsed = adapter.parse_source("").expect("empty source should parse");
            assert_eq!(parsed.source_len, 0);
        }
    }
}

