#[cfg(all(test, feature = "parsing"))]
mod tests {
    use crate::adapters::{LanguageAdapter, java::JavaAdapter};

    #[test]
    fn java_adapter_parses_simple_class() {
        let adapter = JavaAdapter::new();
        let src = r#"
public class HelloWorld {
    public static void main(String[] args) {
        System.out.println("Hello, World!");
    }
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        assert_eq!(parsed.language, "java");
        assert_eq!(parsed.source_len, src.len());

        let syms = adapter.extract_symbols(&parsed).expect("extract_symbols should run");
        assert!(syms.iter().any(|s| s.kind == "class" && s.name == "HelloWorld"));
        assert!(syms.iter().any(|s| s.kind == "method" && s.name == "main"));
    }

    #[test]
    fn java_adapter_handles_empty_source() {
        let adapter = JavaAdapter::new();
        let src = "";
        let parsed = adapter.parse_source(src).expect("parse should succeed on empty");
        assert_eq!(parsed.language, "java");
        assert_eq!(parsed.source_len, 0);
    }

    #[test]
    fn java_adapter_extracts_interface() {
        let adapter = JavaAdapter::new();
        let src = r#"
public interface Repository {
    void save(Object entity);
    Object findById(String id);
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let interfaces: Vec<_> = syms.iter().filter(|s| s.kind == "interface").collect();
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].name, "Repository");
    }

    #[test]
    fn java_adapter_extracts_enum() {
        let adapter = JavaAdapter::new();
        let src = r#"
public enum Status {
    ACTIVE,
    INACTIVE,
    PENDING
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let enums: Vec<_> = syms.iter().filter(|s| s.kind == "enum").collect();
        assert_eq!(enums.len(), 1);
        assert_eq!(enums[0].name, "Status");
    }

    #[test]
    fn java_adapter_extracts_import() {
        let adapter = JavaAdapter::new();
        let src = r#"
import java.util.List;
import java.util.ArrayList;
import static java.util.Collections.emptyList;
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let imports: Vec<_> = syms.iter().filter(|s| s.kind == "import").collect();
        assert!(imports.len() >= 3);
    }

    #[test]
    fn java_adapter_extracts_field() {
        let adapter = JavaAdapter::new();
        let src = r#"
public class Config {
    private String name;
    public int port;
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let fields: Vec<_> = syms.iter().filter(|s| s.kind == "field").collect();
        assert_eq!(fields.len(), 2);
        assert!(fields.iter().any(|f| f.name == "name"));
        assert!(fields.iter().any(|f| f.name == "port"));
    }

    #[test]
    fn java_adapter_extracts_constructor() {
        let adapter = JavaAdapter::new();
        let src = r#"
public class User {
    private String name;
    public User(String name) {
        this.name = name;
    }
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let constructors: Vec<_> = syms.iter().filter(|s| s.kind == "constructor").collect();
        assert_eq!(constructors.len(), 1);
        assert_eq!(constructors[0].name, "User");
    }

    #[test]
    fn java_adapter_multiple_symbols() {
        let adapter = JavaAdapter::new();
        let src = r#"
import java.util.List;

public class UserService {
    private List<String> users;

    public UserService(List<String> users) {
        this.users = users;
    }

    public void addUser(String user) {
        this.users.add(user);
    }

    public List<String> getUsers() {
        return this.users;
    }
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        let kinds: Vec<_> = syms.iter().map(|s| s.kind.as_str()).collect();
        assert!(kinds.contains(&"import"), "kinds: {:?}", kinds);
        assert!(kinds.contains(&"class"), "kinds: {:?}", kinds);
        assert!(kinds.contains(&"constructor"), "kinds: {:?}", kinds);
        assert!(kinds.contains(&"method"), "kinds: {:?}", kinds);
        assert!(kinds.contains(&"field"), "kinds: {:?}", kinds);
        assert!(syms.len() >= 4);
    }

    #[test]
    fn java_adapter_nested_symbols_with_scope() {
        let adapter = JavaAdapter::new();
        let src = r#"
public class Container {
    public void method() {
        System.out.println("hello");
    }
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert!(syms.iter().any(|s| s.kind == "class"));
        assert!(syms.iter().any(|s| s.kind == "method"));
    }

    #[test]
    fn java_adapter_symbol_has_line_range() {
        let adapter = JavaAdapter::new();
        let src = "public void testFunc() {\n    int x = 1;\n    return x + 1;\n}";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert!(!syms.is_empty());
        assert_eq!(syms[0].start_line, 0);
        assert!(syms[0].end_line >= 3);
    }

    #[test]
    fn java_adapter_symbol_has_signature() {
        let adapter = JavaAdapter::new();
        let src = "public int compute(int a, int b) { return a + b; }";
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let syms = adapter.extract_symbols(&parsed).unwrap();
        assert!(syms[0].signature.is_some());
        assert!(syms[0].signature.as_ref().unwrap().contains("compute"));
    }

    #[test]
    fn java_adapter_box_clone() {
        let adapter = JavaAdapter::new();
        let cloned = adapter.box_clone();
        let src = "public class Test {}";
        let parsed = cloned.parse_source(src).expect("clone should work");
        let syms = cloned.extract_symbols(&parsed).unwrap();
        assert!(!syms.is_empty());
    }

    #[test]
    fn java_adapter_source_only_whitespace() {
        let adapter = JavaAdapter::new();
        let src = "   \n\n  \t  ";
        let parsed = adapter.parse_source(src).expect("should not crash on whitespace");
        assert_eq!(parsed.language, "java");
    }

    #[test]
    fn java_adapter_extracts_import_edges() {
        let adapter = JavaAdapter::new();
        let src = r#"
import java.util.List;
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let edges = adapter.extract_imports(&parsed).expect("extract_imports should run");
        assert_eq!(edges.len(), 1);
        let e = &edges[0];
        assert!(e.from_file.contains("<source>") || e.from_file == "<source>");
        assert!(e.to_module.contains("import java.util.List;") || e.to_module.contains("java.util.List"));
        assert_eq!(e.import_kind, "named");
        assert!(!e.resolved);
    }

    #[test]
    fn java_adapter_extracts_call_edges() {
        let adapter = JavaAdapter::new();
        let src = r#"
public class Test {
    public void process() {
        List<String> items = fetch();
        items.size();
    }
}
"#;
        let parsed = adapter.parse_source(src).expect("parse should succeed");
        let edges = adapter.extract_calls(&parsed).expect("extract_calls should run");
        // Should have at least 2 calls: fetch and size
        assert!(edges.len() >= 2);
        // Check that we have fetch call
        let fetch_call = edges.iter().find(|e| e.callee_name == "fetch");
        assert!(fetch_call.is_some());
        // Check that we have size call
        let size_call = edges.iter().find(|e| e.callee_name == "size");
        assert!(size_call.is_some());
    }

    #[test]
    fn java_adapter_registers_to_registry() {
        let registry = crate::app::bootstrap::Registry::new();
        crate::adapters::java::register_to(&registry);
        assert!(registry.get("java").is_some());
        let langs = registry.list_languages();
        assert!(langs.contains(&"java".to_string()));
    }
}
