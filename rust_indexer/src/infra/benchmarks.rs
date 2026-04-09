use std::time::Instant;

use crate::adapters::LanguageAdapter;
use crate::domain::parser::ParsedFile;
use crate::domain::types::Symbol;
use crate::infra::parser_pool::ParserPool;
use anyhow::Result;

pub struct BenchResult {
    pub elapsed_us: u128,
    pub symbols_count: usize,
    pub source_len: usize,
    pub language: String,
    pub iterations: usize,
}

pub fn parse_once(
    adapter: &dyn LanguageAdapter,
    source: &str,
) -> Result<(ParsedFile, Vec<Symbol>)> {
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
            Ok((_parsed, symbols)) => {
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
    use crate::adapters::java::JavaAdapter;
    use crate::adapters::rust::RustAdapter;
    use crate::adapters::typescript::TypeScriptAdapter;
    use rayon::prelude::*;
    use std::sync::Arc;

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
        assert!(
            elapsed.as_millis() < 500,
            "parse latency too high: {:?}",
            elapsed
        );
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
        assert!(
            elapsed.as_millis() < 1000,
            "medium source latency too high: {:?}",
            elapsed
        );
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
        assert!(
            elapsed.as_millis() < 2000,
            "large source latency too high: {:?}",
            elapsed
        );
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
        assert!(
            elapsed.as_millis() < 500,
            "TS parse latency too high: {:?}",
            elapsed
        );
    }

    #[test]
    fn bench_parse_latency_typescript_large_source() {
        let pool = make_pool();
        let source = generate_source_repetitions(&ts_pattern(0), 50);
        let start = Instant::now();
        for _ in 0..5 {
            let result = parse_once(pool.get("typescript").unwrap().as_ref(), &source).unwrap();
            assert!(
                !result.1.is_empty(),
                "TypeScript should extract symbols from large source"
            );
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 2000,
            "TS large source latency too high: {:?}",
            elapsed
        );
    }

    // --- Happy path: Java ---

    #[test]
    fn bench_parse_latency_java_small_source() {
        let pool = make_pool();
        let source = "class Foo { void bar() {} }";
        let start = Instant::now();
        for _ in 0..100 {
            let result = parse_once(pool.get("java").unwrap().as_ref(), source).unwrap();
            assert!(!result.1.is_empty());
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 500,
            "Java parse latency too high: {:?}",
            elapsed
        );
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
        assert!(
            elapsed.as_millis() < 2000,
            "Java large source latency too high: {:?}",
            elapsed
        );
    }

    // --- Throughput tests ---

    #[test]
    fn bench_throughput_rust_parser() {
        let pool = make_pool();
        let source = generate_source_repetitions(&rust_pattern(0), 10);
        let result =
            benchmark_adapter(&pool, "rust", &source, ITERATIONS).expect("rust should succeed");

        assert_eq!(result.iterations, ITERATIONS);
        assert!(result.symbols_count > 0);
        let symbols_per_second =
            (result.symbols_count as f64) / (result.elapsed_us as f64) * 1_000_000.0;
        eprintln!(
            "Rust throughput: {:.0} symbols/s ({}/{})",
            symbols_per_second, result.symbols_count, result.source_len
        );
        assert!(
            symbols_per_second > 100.0,
            "throughput too low: {:.0}",
            symbols_per_second
        );
    }

    #[test]
    fn bench_throughput_typescript_parser() {
        let pool = make_pool();
        let source = generate_source_repetitions(&ts_pattern(0), 5);
        let result = benchmark_adapter(&pool, "typescript", &source, ITERATIONS)
            .expect("typescript should succeed");

        assert_eq!(result.iterations, ITERATIONS);
        assert!(result.symbols_count > 0);
        let symbols_per_second =
            (result.symbols_count as f64) / (result.elapsed_us as f64) * 1_000_000.0;
        eprintln!(
            "TypeScript throughput: {:.0} symbols/s ({}/{})",
            symbols_per_second, result.symbols_count, result.source_len
        );
        assert!(
            symbols_per_second > 100.0,
            "throughput too low: {:.0}",
            symbols_per_second
        );
    }

    #[test]
    fn bench_throughput_java_parser() {
        let pool = make_pool();
        let source = generate_source_repetitions(&java_pattern(0), 10);
        let result =
            benchmark_adapter(&pool, "java", &source, ITERATIONS).expect("java should succeed");

        assert_eq!(result.iterations, ITERATIONS);
        assert!(result.symbols_count > 0);
        let symbols_per_second =
            (result.symbols_count as f64) / (result.elapsed_us as f64) * 1_000_000.0;
        eprintln!(
            "Java throughput: {:.0} symbols/s ({}/{})",
            symbols_per_second, result.symbols_count, result.source_len
        );
        assert!(
            symbols_per_second > 100.0,
            "throughput too low: {:.0}",
            symbols_per_second
        );
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
                    format!("fn func_{i}() {{}} class Class_{i} {{}} interface I_{i} {{}}",),
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
        assert!(
            elapsed.as_millis() < 5000,
            "parallel parsing too slow: {:?}",
            elapsed
        );
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

        assert!(
            parallel_elapsed < serial_elapsed * 10,
            "parallel should not be 10x slower"
        );
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

    // --- Large-scale benchmark with 100-1k files ---

    #[test]
    fn bench_index_100_files_all_languages() {
        use tempfile::TempDir;

        let dir = TempDir::new().expect("tempdir");
        create_file_set(dir.path(), 100);

        let pool = Arc::new(make_pool());
        let start = Instant::now();

        let mut total_symbols = 0;
        let mut total_files = 0;

        for entry in walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let lang = match ext {
                "rs" => "rust",
                "ts" | "js" => "typescript",
                "java" => "java",
                _ => continue,
            };

            let source = std::fs::read_to_string(path).unwrap();
            let adapter = pool.get(lang).unwrap();
            let parsed = adapter.parse_source(&source).unwrap();
            let symbols = adapter.extract_symbols(&parsed).unwrap();
            total_symbols += symbols.len();
            total_files += 1;
        }

        let elapsed = start.elapsed();

        eprintln!(
            "100-file benchmark: {} files, {} symbols, elapsed: {:?}",
            total_files, total_symbols, elapsed
        );

        assert_eq!(total_files, 100, "should process exactly 100 files");
        assert!(total_symbols > 0, "should extract symbols from all files");
        assert!(
            elapsed.as_secs() < 30,
            "indexing 100 files took too long: {:?}",
            elapsed
        );
    }

    #[test]
    fn bench_index_500_files_parallel() {
        use tempfile::TempDir;

        let dir = TempDir::new().expect("tempdir");
        create_file_set(dir.path(), 500);

        let start = Instant::now();

        let entries: Vec<_> = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .collect();

        let results: Vec<_> = entries
            .par_iter()
            .map(|entry| {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let lang = match ext {
                    "rs" => "rust",
                    "ts" | "js" => "typescript",
                    "java" => "java",
                    _ => return 0,
                };

                let source = std::fs::read_to_string(path).unwrap();
                let file_pool = Arc::new(make_pool());
                let adapter = file_pool.get(lang).unwrap();
                let parsed = adapter.parse_source(&source).unwrap();
                adapter.extract_symbols(&parsed).unwrap().len()
            })
            .collect();

        let elapsed = start.elapsed();
        let total_symbols: usize = results.iter().sum();
        let total_files = results.len();

        eprintln!(
            "500-file parallel benchmark: {} files, {} symbols, elapsed: {:?}",
            total_files, total_symbols, elapsed
        );

        assert!(results.len() >= 500, "should process at least 500 files");
        assert!(total_symbols > 0, "should extract symbols");
        assert!(
            elapsed.as_secs() < 60,
            "parallel indexing 500 files took too long: {:?}",
            elapsed
        );
    }

    #[test]
    fn bench_serial_vs_parallel_comparison() {
        use tempfile::TempDir;

        let dir = TempDir::new().expect("tempdir");
        create_file_set(dir.path(), 200);

        let entries: Vec<_> = walkdir::WalkDir::new(dir.path())
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .collect();

        // Serial execution
        let serial_start = Instant::now();
        let serial_symbols: usize = entries
            .iter()
            .filter_map(|entry| {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str())?;
                let lang = match ext {
                    "rs" => "rust",
                    "ts" | "js" => "typescript",
                    "java" => "java",
                    _ => return None,
                };

                let source = std::fs::read_to_string(path).ok()?;
                let pool = make_pool();
                let adapter = pool.get(lang)?;
                let parsed = adapter.parse_source(&source).ok()?;
                Some(adapter.extract_symbols(&parsed).ok()?.len())
            })
            .sum();
        let serial_elapsed = serial_start.elapsed();

        // Parallel execution
        let parallel_start = Instant::now();
        let parallel_symbols: usize = entries
            .par_iter()
            .filter_map(|entry| {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str())?;
                let lang = match ext {
                    "rs" => "rust",
                    "ts" | "js" => "typescript",
                    "java" => "java",
                    _ => return None,
                };

                let source = std::fs::read_to_string(path).ok()?;
                let pool = Arc::new(make_pool());
                let adapter = pool.get(lang)?;
                let parsed = adapter.parse_source(&source).ok()?;
                Some(adapter.extract_symbols(&parsed).ok()?.len())
            })
            .sum();
        let parallel_elapsed = parallel_start.elapsed();

        eprintln!(
            "Serial vs Parallel (200 files): {:?} vs {:?} (speedup: {:.2}x), symbols: {} vs {}",
            serial_elapsed,
            parallel_elapsed,
            serial_elapsed.as_micros() as f64 / parallel_elapsed.as_micros().max(1) as f64,
            serial_symbols,
            parallel_symbols
        );

        assert_eq!(
            serial_symbols, parallel_symbols,
            "both should find same symbols"
        );
        assert!(
            parallel_elapsed <= serial_elapsed * 2,
            "parallel should not be significantly slower"
        );
    }

    #[test]
    fn bench_throughput_scales_with_file_count() {
        use tempfile::TempDir;

        let counts = vec![50, 100, 200];
        let mut results = Vec::new();

        for count in counts {
            let dir = TempDir::new().expect("tempdir");
            create_file_set(dir.path(), count);

            let entries: Vec<_> = walkdir::WalkDir::new(dir.path())
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .collect();

            let start = Instant::now();
            let total_symbols: usize = entries
                .par_iter()
                .filter_map(|entry| {
                    let path = entry.path();
                    let ext = path.extension().and_then(|e| e.to_str())?;
                    let lang = match ext {
                        "rs" => "rust",
                        "ts" | "js" => "typescript",
                        "java" => "java",
                        _ => return None,
                    };

                    let source = std::fs::read_to_string(path).ok()?;
                    let pool = Arc::new(make_pool());
                    let adapter = pool.get(lang)?;
                    let parsed = adapter.parse_source(&source).ok()?;
                    Some(adapter.extract_symbols(&parsed).ok()?.len())
                })
                .sum();
            let elapsed_us = start.elapsed().as_micros();

            let files_per_second = (count as f64) / (elapsed_us as f64) * 1_000_000.0;
            let symbols_per_second = (total_symbols as f64) / (elapsed_us as f64) * 1_000_000.0;

            eprintln!(
                "{} files: {} symbols, {:.0} files/s, {:.0} symbols/s",
                count, total_symbols, files_per_second, symbols_per_second
            );

            results.push((count, files_per_second, symbols_per_second));
        }

        // Verify scaling is reasonable (not linear degradation)
        assert!(results.len() == 3, "should have 3 data points");
        for (_, fps, _) in &results {
            assert!(*fps > 1.0, "throughput too low: {:.0} files/s", fps);
        }
    }

    /// Helper to create a set of source files across multiple languages
    fn create_file_set(base_dir: &std::path::Path, count: usize) {
        #[allow(clippy::type_complexity)]
        let langs: Vec<(&str, &str, Box<dyn Fn(usize) -> String>)> = vec![
            (
                "rs",
                "rust",
                Box::new(|i| {
                    format!(
                        "fn function_{}(a: u32, b: u32) -> u32 {{ a + b }}\n\
                     struct Struct_{} {{ value: u32, name: String }}\n\
                     pub mod module_{} {{\n    pub fn helper() {{}}\n}}\n\n",
                        i, i, i
                    )
                }),
            ),
            (
                "ts",
                "typescript",
                Box::new(|i| {
                    format!(
                    "function process_{}(data: any) {{ return data; }}\n\
                     class Service_{} {{\n    private items: any[] = [];\n    add(item: any) {{ this.items.push(item); }}\n}}\n\n",
                    i, i
                )
                }),
            ),
            (
                "java",
                "java",
                Box::new(|i| {
                    format!(
                    "public class Repository_{} {{\n    private String name;\n    public void save(Object entity) {{}}\n    private List<Object> findAll() {{ return null; }}\n}}\n\n",
                    i
                )
                }),
            ),
        ];

        for i in 0..count {
            let (ext, _lang, generator) = &langs[i % langs.len()];
            let subdir = match i % 3 {
                0 => "src",
                1 => "lib",
                _ => "core",
            };

            let dir = base_dir.join(subdir);
            std::fs::create_dir_all(&dir).expect("create subdir");

            let filename = format!("file_{}.{}", i, ext);
            let content = generator(i);
            std::fs::write(dir.join(filename), content).expect("write file");
        }
    }
}
