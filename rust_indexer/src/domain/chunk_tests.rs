#[cfg(test)]
mod tests {
    use crate::domain::types::Chunk;
    use serde_json::json;

    #[test]
    fn chunk_validation_success() {
        let c = Chunk {
            id: "chk-1".into(),
            file_path: "src/lib.rs".into(),
            start_line: 1,
            end_line: 10,
            content: "fn foo() {}".into(),
            text: "fn foo() {}".into(),
            md5: "abc".into(),
            size: 100,
            language: Some("rust".into()),
            symbol_id: Some("sym1".into()),
            symbol_ids: vec!["sym1".into()],
            chunk_kind: Some("Symbol".into()),
            metadata: Some(std::collections::HashMap::from([
                ("tokens".into(), json!(120)),
            ])),
        };

        assert!(c.validate().is_ok());
        assert_eq!(format!("{}", c), "Chunk { id: chk-1, file: src/lib.rs, lines: 1-10 }");
    }

    #[test]
    fn chunk_validation_failure_zero_line() {
        let c = Chunk {
            id: "chk-2".into(),
            file_path: "src/lib.rs".into(),
            start_line: 0,
            end_line: 0,
            content: "".into(),
            text: "".into(),
            md5: "".into(),
            size: 0,
            language: None,
            symbol_id: None,
            symbol_ids: vec![],
            chunk_kind: None,
            metadata: None,
        };

        assert!(c.validate().is_err());
    }

    #[test]
    fn chunk_validation_failure_start_greater_end() {
        let c = Chunk {
            id: "chk-3".into(),
            file_path: "src/lib.rs".into(),
            start_line: 10,
            end_line: 1,
            content: "".into(),
            text: "".into(),
            md5: "".into(),
            size: 0,
            language: None,
            symbol_id: None,
            symbol_ids: vec![],
            chunk_kind: None,
            metadata: None,
        };

        assert!(c.validate().is_err());
    }
}
