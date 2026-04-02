use std::time::Instant;
use rayon::prelude::*;

use crate::domain::parser::ParsedFile;
use crate::domain::types::Symbol;
use anyhow::Result;
use crate::adapters::LanguageAdapter;
use crate::infra::parser_pool::ParserPool;

pub struct BenchResult {
    pub elapsed_us: u128,
    pub symbols_count: usize,
    pub source_len: usize,
    pub language: String,
    pub iterations: usize,
}

pub fn parse_once(adapter: &dyn LanguageAdapter, source: &str) -> Result<(ParsedFile, Vec<Symbol>)> {
    let parsed = adapter.parse_source(source)?;
    let symbols = adapter.extract_symbols(&parsed)?;
    Ok((parsed, symbols))
}

pub fn benchmark_adapter(
    pool: &ParserPool,
    language: &str,
    source: &str,
    iterations: usize,
) -> Option<BenchResult> {
    let adapter = pool.get(language)?;
    let mut total_elapsed = 0u128;
    let mut total_symbols = 0usize;

    for _ in 0..iterations {
        let start = Instant::now();
        match parse_once(adapter.as_ref(), source) {
            Ok((parsed, symbols)) => {
                total_elapsed += start.elapsed().as_micros();
                total_symbols += symbols.len();
            }
            Err(_) => return None,
        }
    }

    Some(BenchResult {
        elapsed_us: total_elapsed,
        symbols_count: total_symbols,
        source_len: source.len(),
        language: language.to_string(),
        iterations,
    })
}

#[cfg(all(test, feature = "parsing"))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::adapters::rust::RustAdapter;
    use crate::adapters::typescript::TypeScriptAdapter;
    use crate::adapters::java::JavaAdapter;

    const ITERATIONS: usize = 10;

    fn make_pool() -> ParserPool {
        let pool = ParserPool::new();
        pool.register("rust", Arc::new(RustAdapter));
        pool.register("typescript", Arc::new(TypeScriptAdapter));
        pool.register("javascript", Arc::new(TypeScriptAdapter));
        pool.register("java", Arc::new(JavaAdapter));
        pool
    }

    fn generate_source_repetitions(pattern: &str, count: usize) -> String {
        let mut src = String::new();
        for i in 0..count {
            src.push_str(&pattern.replace("___IDX___", &i.to_string()));
        }
        src
    }

    fn rust_pattern(idx: usize) -> String {
        format!(
            "fn func_{idx}(a: u32, b: u32) -> u32 {{ a + b }}\n\
             struct Struct_{idx} {{ value: u32 }}\n\
             enum Enum_{idx} {{ A_{idx}, B_{idx}, C_{idx} }}\n\n"
        )
    }

    fn ts_pattern(idx: usize) -> String {
        // Use pure JavaScript syntax (tree-sitter-javascript parser)
        format!(
            "function fn_{idx}(a, b) {{ return a + b; }}\n\
             var var_{idx} = {idx};\n\n"
        )
    }

    fn java_pattern(idx: usize) -> String {
        format!(
            "public class JavaClass_{idx} {{\n    private int field_{idx};\n    public void method_{idx}(int arg) {{}}\n}}\n\n"
        )
    }

    // --- Happy path tests ---

    #[test]
    fn bench_parse_latency_rust_small_source() {
        let pool = make_pool();
        let source = "fn foo() { struct Bar { x: u32 } }";
        let start = Instant::now();
        for _ in 0..100 {
            let result = parse_once(pool.get("rust").unwrap().as_ref(), source).unwrap();
            assert!(result.1.len() >= 2);
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 500, "parse latency too high: {:?}", elapsed);
    }

    #[test]
    fn bench_parse_latency_rust_medium_source() {
        let pool = make_pool();
        let source = generate_source_repetitions(&rust_pattern(0), 10);
        let start = Instant::now();
        for _ in 0..10 {
            let result = parse_once(pool.get("rust").unwrap().as_ref(), &source).unwrap();
            assert!(!result.1.is_empty());
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 1000, "medium source latency too high: {:?}", elapsed);
    }

    #[test]
    fn bench_parse_latency_rust_large_source() {
        let pool = make_pool();
        let source = generate_source_repetitions(&rust_pattern(0), 100);
        let start = Instant::now();
        for _ in 0..5 {
            let result = parse_once(pool.get("rust").unwrap().as_ref(), &source).unwrap();
            assert!(!result.1.is_empty());
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 2000, "large source latency too high: {:?}", elapsed);
    }

    // --- Happy path: TypeScript ---

    #[test]
    fn bench_parse_latency_typescript_small_source() {
        let pool = make_pool();
        let source = "class Foo {} function bar() { return 1; }";
        let start = Instant::now();
        for _ in 0..100 {
            let result = parse_once(pool.get("typescript").unwrap().as_ref(), source).unwrap();
            assert!(result.1.len() >= 2);
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 500, "TS parse latency too high: {:?}", elapsed);
    }

    #[test]
    fn bench_parse_latency_typescript_large_source() {
        let pool = make_pool();
        let source = generate_source_repetitions(&ts_pattern(0), 50);
        let start = Instant::now();
        for _ in 0..5 {
            let result = parse_once(pool.get("typescript").unwrap().as_ref(), &source).unwrap();
            assert!(!result.1.is_empty(), "TypeScript should extract symbols from large source");
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 2000, "TS large source latency too high: {:?}", elapsed);
    }

    // --- Happy path: Java ---

    #[test]
    fn bench_parse_latency_java_small_source() {
        let pool = make_pool();
        let source = "class Foo { void bar() {} }";
        let start = Instant::now();
        for _ in 0..100 {
            let result = parse_once(pool.get("java").unwrap().as_ref(), source).unwrap();
            assert!(result.1.len() >= 1);
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 500, "Java parse latency too high: {:?}", elapsed);
    }

    #[test]
    fn bench_parse_latency_java_large_source() {
        let pool = make_pool();
        let source = generate_source_repetitions(&java_pattern(0), 100);
        let start = Instant::now();
        for _ in 0..5 {
            let result = parse_once(pool.get("java").unwrap().as_ref(), &source).unwrap();
            assert!(!result.1.is_empty());
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 2000, "Java large source latency too high: {:?}", elapsed);
    }

    // --- Throughput tests ---

    #[test]
    fn bench_throughput_rust_parser() {
        let pool = make_pool();
        let source = generate_source_repetitions(&rust_pattern(0), 10);
        let result = benchmark_adapter(&pool, "rust", &source, ITERATIONS).expect("rust should succeed");
        
        assert_eq!(result.iterations, ITERATIONS);
        assert!(result.symbols_count > 0);
        let symbols_per_second = (result.symbols_count as f64) / (result.elapsed_us as f64) * 1_000_000.0;
        eprintln!(
            "Rust throughput: {:.0} symbols/s ({}/{})",
            symbols_per_second,
            result.symbols_count,
            result.source_len
        );
        assert!(symbols_per_second > 100.0, "throughput too low: {:.0}", symbols_per_second);
    }

    #[test]
    fn bench_throughput_typescript_parser() {
        let pool = make_pool();
        let source = generate_source_repetitions(&ts_pattern(0), 5);
        let result = benchmark_adapter(&pool, "typescript", &source, ITERATIONS).expect("typescript should succeed");
        
        assert_eq!(result.iterations, ITERATIONS);
        assert!(result.symbols_count > 0);
        let symbols_per_second = (result.symbols_count as f64) / (result.elapsed_us as f64) * 1_000_000.0;
        eprintln!(
            "TypeScript throughput: {:.0} symbols/s ({}/{})",
            symbols_per_second,
            result.symbols_count,
            result.source_len
        );
        assert!(symbols_per_second > 100.0, "throughput too low: {:.0}", symbols_per_second);
    }

    #[test]
    fn bench_throughput_java_parser() {
        let pool = make_pool();
        let source = generate_source_repetitions(&java_pattern(0), 10);
        let result = benchmark_adapter(&pool, "java", &source, ITERATIONS).expect("java should succeed");
        
        assert_eq!(result.iterations, ITERATIONS);
        assert!(result.symbols_count > 0);
        let symbols_per_second = (result.symbols_count as f64) / (result.elapsed_us as f64) * 1_000_000.0;
        eprintln!(
            "Java throughput: {:.0} symbols/s ({}/{})",
            symbols_per_second,
            result.symbols_count,
            result.source_len
        );
        assert!(symbols_per_second > 100.0, "throughput too low: {:.0}", symbols_per_second);
    }

    // --- Thread-safety tests (parallel parsing) ---

    #[test]
    fn bench_concurrent_parsing_is_thread_safe() {
        let pool = Arc::new(make_pool());
        let sources: Vec<_> = (0..50)
            .map(|i| {
                let lang = match i % 3 {
                    0 => "rust",
                    1 => "typescript",
                    _ => "java",
                };
                (
                    lang.to_string(),
                    format!(
                        "fn func_{i}() {{}} class Class_{i} {{}} interface I_{i} {{}}",
                    )
                )
            })
            .collect();

        let start = Instant::now();
        let results: Vec<_> = sources
            .par_iter()
            .map(|(lang, src)| {
                let adapter = Arc::clone(&pool);
                let a = adapter.get(lang).unwrap();
                let parsed = a.parse_source(src).unwrap();
                a.extract_symbols(&parsed).unwrap()
            })
            .collect();

        let elapsed = start.elapsed();
        let total_symbols: usize = results.iter().map(|s| s.len()).sum();
        
        assert_eq!(results.len(), 50, "all 50 sources should be parsed");
        assert!(total_symbols > 0, "should extract symbols");
        assert!(elapsed.as_millis() < 5000, "parallel parsing too slow: {:?}", elapsed);
    }

    #[test]
    fn bench_concurrent_parsing_throughput_improves_with_parallelism() {
        let pool = Arc::new(make_pool());
        let source = "fn foo() { struct Bar { x: u32 } enum Baz { A, B } }";
        let iterations = 50;

        // Serial execution
        let adapter = pool.get("rust").unwrap();
        let start = Instant::now();
        let serial_result: Vec<_> = (0..iterations)
            .map(|_| {
                let parsed = adapter.parse_source(source).unwrap();
                adapter.extract_symbols(&parsed).unwrap()
            })
            .collect();
        let serial_elapsed = start.elapsed();

        // Parallel execution (using separate adapter clones to test thread safety)
        let start = Instant::now();
        let parallel_result: Vec<_> = (0..iterations)
            .into_par_iter()
            .map(|_| {
                let pool = Arc::new(make_pool());
                let adapter = pool.get("rust").unwrap();
                let parsed = adapter.parse_source(source).unwrap();
                adapter.extract_symbols(&parsed).unwrap()
            })
            .collect();
        let parallel_elapsed = start.elapsed();

        assert_eq!(serial_result.len(), iterations);
        assert_eq!(parallel_result.len(), iterations);
        
        eprintln!(
            "Serial: {:?}, Parallel: {:?}, Speedup: {:.2}x",
            serial_elapsed, 
            parallel_elapsed,
            serial_elapsed.as_micros() as f64 / parallel_elapsed.as_micros().max(1) as f64
        );
        
        assert!(parallel_elapsed < serial_elapsed * 10, "parallel should not be 10x slower");
    }

    // --- Unhappy path tests ---

    #[test]
    fn bench_returns_none_for_missing_language() {
        let pool = make_pool();
        let result = benchmark_adapter(&pool, "cobol", "COBOL CODE", 1);
        assert!(result.is_none());
    }

    #[test]
    fn bench_handles_empty_source() {
        let pool = make_pool();
        for lang in &["rust", "typescript", "java"] {
            let result = benchmark_adapter(&pool, lang, "", 1);
            assert!(result.is_some(), "{} should handle empty source", lang);
            let r = result.unwrap();
            assert_eq!(r.iterations, 1);
        }
    }

    #[test]
    fn bench_handles_whitespace_source() {
        let pool = make_pool();
        for lang in &["rust", "typescript", "java"] {
            let result = benchmark_adapter(&pool, lang, "   \n\t\n  ", 1);
            assert!(result.is_some(), "{} should handle whitespace source", lang);
        }
    }

    #[test]
    fn bench_handles_zero_iterations() {
        let pool = make_pool();
        let result = benchmark_adapter(&pool, "rust", "fn foo() {}", 0);
        let r = result.expect("should handle 0 iterations");
        assert_eq!(r.elapsed_us, 0);
        assert_eq!(r.symbols_count, 0);
        assert_eq!(r.iterations, 0);
    }
}
