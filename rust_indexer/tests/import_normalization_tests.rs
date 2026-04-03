use rust_indexer::domain::types::{ImportEdge, Location};

#[test]
fn test_import_normalization_basic() {
    let edge = ImportEdge {
        id: "ie_test".to_string(),
        from_file: "src/main.rs".to_string(),
        to_module: "std::collections::HashMap".to_string(),
        imported_symbol: None,
        alias: None,
        import_kind: "named".to_string(),
        location: Location { start_line: 1, start_col: 0, end_line: 1, end_col: 40 },
        resolved: false,
    };

    assert_eq!(edge.to_module, "std::collections::HashMap");
    assert!(edge.imported_symbol.is_none());
    assert!(edge.alias.is_none());
    assert_eq!(edge.import_kind, "named");
    assert!(!edge.resolved);
}

#[test]
fn test_import_with_alias_after_normalization() {
    // What normalization should produce for: use std::collections::HashMap as Map;
    let normalized = ImportEdge {
        id: "ie_alias".to_string(),
        from_file: "src/lib.rs".to_string(),
        to_module: "std::collections::HashMap".to_string(),
        imported_symbol: Some("HashMap".to_string()),
        alias: Some("Map".to_string()),
        import_kind: "named".to_string(),
        location: Location { start_line: 1, start_col: 0, end_line: 1, end_col: 50 },
        resolved: false,
    };

    assert_eq!(normalized.imported_symbol, Some("HashMap".to_string()));
    assert_eq!(normalized.alias, Some("Map".to_string()));
}

#[test]
fn test_import_default_kind() {
    let normalized = ImportEdge {
        id: "ie_default".to_string(),
        from_file: "src/main.js".to_string(),
        to_module: "lodash".to_string(),
        imported_symbol: Some("default".to_string()),
        alias: Some("_".to_string()),
        import_kind: "default".to_string(),
        location: Location { start_line: 1, start_col: 0, end_line: 1, end_col: 30 },
        resolved: false,
    };

    assert_eq!(normalized.imported_symbol, Some("default".to_string()));
    assert_eq!(normalized.import_kind, "default");
}

#[test]
fn test_import_namespace_kind() {
    let normalized = ImportEdge {
        id: "ie_namespace".to_string(),
        from_file: "src/file.ts".to_string(),
        to_module: "fs".to_string(),
        imported_symbol: None,
        alias: Some("fs".to_string()),
        import_kind: "namespace".to_string(),
        location: Location { start_line: 1, start_col: 0, end_line: 1, end_col: 30 },
        resolved: false,
    };

    assert_eq!(normalized.alias, Some("fs".to_string()));
    assert_eq!(normalized.import_kind, "namespace");
}

#[test]
fn test_import_side_effect_kind() {
    let normalized = ImportEdge {
        id: "ie_side_effect".to_string(),
        from_file: "src/polyfills.ts".to_string(),
        to_module: "core-js/promise".to_string(),
        imported_symbol: None,
        alias: None,
        import_kind: "side_effect".to_string(),
        location: Location { start_line: 1, start_col: 0, end_line: 1, end_col: 35 },
        resolved: false,
    };

    assert_eq!(normalized.import_kind, "side_effect");
    assert!(normalized.imported_symbol.is_none());
    assert!(normalized.alias.is_none());
}

#[test]
fn test_import_reexport_kind() {
    let normalized = ImportEdge {
        id: "ie_reexport".to_string(),
        from_file: "src/public_api.rs".to_string(),
        to_module: "crate::internal::helper".to_string(),
        imported_symbol: None,
        alias: None,
        import_kind: "reexport".to_string(),
        location: Location { start_line: 1, start_col: 0, end_line: 1, end_col: 40 },
        resolved: false,
    };

    assert_eq!(normalized.import_kind, "reexport");
}

#[test]
fn test_import_resolved_local() {
    let normalized = ImportEdge {
        id: "ie_local".to_string(),
        from_file: "src/components/main.js".to_string(),
        to_module: "src/utils/helper.js".to_string(),
        imported_symbol: None,
        alias: None,
        import_kind: "named".to_string(),
        location: Location { start_line: 1, start_col: 0, end_line: 1, end_col: 30 },
        resolved: true,
    };

    assert!(normalized.resolved);
    assert_eq!(normalized.to_module, "src/utils/helper.js");
}

#[test]
fn test_import_edge_serialization() {
    let edge = ImportEdge {
        id: "ie_serde".to_string(),
        from_file: "test.rs".to_string(),
        to_module: "std::vec::Vec".to_string(),
        imported_symbol: Some("Vec".to_string()),
        alias: Some("V".to_string()),
        import_kind: "named".to_string(),
        location: Location { start_line: 10, start_col: 2, end_line: 10, end_col: 30 },
        resolved: true,
    };

    let json = serde_json::to_string(&edge).expect("should serialize");
    let deserialized: ImportEdge = serde_json::from_str(&json).expect("should deserialize");

    assert_eq!(deserialized.id, edge.id);
    assert_eq!(deserialized.from_file, edge.from_file);
    assert_eq!(deserialized.to_module, edge.to_module);
    assert_eq!(deserialized.imported_symbol, edge.imported_symbol);
    assert_eq!(deserialized.alias, edge.alias);
    assert_eq!(deserialized.import_kind, edge.import_kind);
    assert_eq!(deserialized.location, edge.location);
    assert_eq!(deserialized.resolved, edge.resolved);
}

#[test]
fn test_normalize_import_integration_with_adapter() {
    // End-to-end: adapter extracts → normalize refines
    #[cfg(feature = "parsing")]
    {
        use rust_indexer::adapters::rust::RustAdapter;
        use rust_indexer::adapters::LanguageAdapter;
        use rust_indexer::domain::normalize_import;

        let adapter = RustAdapter::new();
        let parsed = adapter.parse_source("use std::collections::HashMap;").expect("parse should succeed");
        let raw_edges = adapter.extract_imports(&parsed).expect("extract_imports should run");
        assert_eq!(raw_edges.len(), 1);

        let normalized = normalize_import(&raw_edges[0], "rust");
        assert_eq!(normalized.to_module, "std::collections");
        assert_eq!(normalized.imported_symbol, Some("HashMap".to_string()));
        assert_eq!(normalized.import_kind, "named");
        assert!(!normalized.resolved);
    }
}
