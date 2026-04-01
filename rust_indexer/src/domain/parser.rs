// Placeholder for Tree-sitter parsing domain service

pub struct ParsedFile {
    pub language: String,
    pub source_len: usize,
}

pub fn parse_source(language: &str, source: &str) -> ParsedFile {
    ParsedFile {
        language: language.to_string(),
        source_len: source.len(),
    }
}
