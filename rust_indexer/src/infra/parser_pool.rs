// Simple ParserPool using tree-sitter parsers per thread

use std::sync::Arc;
use std::sync::Mutex;

use tree_sitter::{Parser, Language};
use tree_sitter_rust;

pub struct ParserPool {
    // pool of parsers protected by a mutex for simple reuse
    parsers: Vec<Arc<Mutex<Parser>>>,
}

impl ParserPool {
    pub fn new(size: usize) -> Self {
        let mut parsers = Vec::with_capacity(size);
        for _ in 0..size {
            let mut parser = Parser::new();
            // get Rust language from the tree-sitter-rust crate
            let lang: Language = tree_sitter_rust::language();
            let _ = parser.set_language(lang);
            parsers.push(Arc::new(Mutex::new(parser)));
        }
        ParserPool { parsers }
    }

    pub fn get(&self, idx: usize) -> Arc<Mutex<Parser>> {
        let i = idx % self.parsers.len();
        Arc::clone(&self.parsers[i])
    }
}
