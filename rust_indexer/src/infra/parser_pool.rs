// Simple ParserPool using tree-sitter parsers per thread

use std::sync::Arc;
use std::sync::Mutex;

use tree_sitter::Parser;
#[cfg(feature = "parsing")] use tree_sitter_rust;

pub struct ParserPool {
    // pool of parsers protected by a mutex for simple reuse
    parsers: Vec<Arc<Mutex<Parser>>>,
}

impl ParserPool {
    pub fn new(size: usize) -> Self {
        let mut parsers = Vec::with_capacity(size);
        for _ in 0..size {
            let parser = Parser::new();
            // when parsing feature enabled, set the language; otherwise leave parser unconfigured
            #[cfg(feature = "parsing")] {
                let lang: Language = tree_sitter_rust::language();
                let _ = parser.set_language(lang);
            }
            parsers.push(Arc::new(Mutex::new(parser)));
        }
        ParserPool { parsers }
    }

    pub fn get(&self, idx: usize) -> Arc<Mutex<Parser>> {
        let i = idx % self.parsers.len();
        Arc::clone(&self.parsers[i])
    }
}

#[cfg(all(test, feature = "parsing"))]
mod tests {
    use super::*;

    #[test]
    fn parser_pool_parses_simple_rust() {
        let pool = ParserPool::new(2);
        let parser_arc = pool.get(0);
        let mut parser = parser_arc.lock().unwrap();

        let src = "fn main() { println!(\"hello\"); }";
        let tree = parser.parse(src, None);
        assert!(tree.is_some());
    }
}

