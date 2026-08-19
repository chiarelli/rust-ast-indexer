use std::sync::mpsc;
use std::sync::Arc;

use rayon::prelude::*;
use std::collections::HashMap;

use crate::app::bootstrap::ApplicationContext;
use crate::application::chunking::{
    ChunkStrategy, ChunkingOptions, ChunkingStrategy, ContextInjectionChunker, LineLimitedChunker,
    OverlapChunker, SemanticChunker, SymbolBoundaryChunker,
};
use crate::domain::normalize::normalize_import;
use crate::domain::types::{Chunk, FileRecord, Symbol};
use crate::infra::backpressure::{BackpressureConfig, BackpressureMonitor};
use crate::infra::parser_pool::ParserPool;
use crate::infra::walker::{walk_path, ScanOptions, WalkerError};

fn detect_language(path: &str) -> Option<String> {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str());

    match ext {
        Some("rs") => Some("rust".to_string()),
        Some("ts") | Some("tsx") => Some("typescript".to_string()),
        Some("js") | Some("jsx") => Some("javascript".to_string()),
        Some("java") => Some("java".to_string()),
        Some("go") => Some("go".to_string()),
        Some("py") => Some("python".to_string()),
        Some(_) => None,
        None => None,
    }
}

pub struct IndexOptions {
    pub max_concurrency: usize,
    pub explicit_files: Option<Vec<String>>,
    pub extract_imports: bool,
    pub extract_calls: bool,
    pub chunking: ChunkingOptions,
    pub backpressure: Option<BackpressureConfig>,
}

pub struct Indexer {
    ctx: Option<Arc<ApplicationContext>>,
}

pub struct IndexResult {
    pub chunks: Vec<Chunk>,
    pub files: Vec<FileRecord>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            max_concurrency: 1,
            explicit_files: None,
            extract_imports: true,
            extract_calls: true,
            chunking: ChunkingOptions::default(),
            backpressure: None,
        }
    }
}

impl Default for Indexer {
    fn default() -> Self {
        Self::new()
    }
}

impl Indexer {
    pub fn new() -> Self {
        Self { ctx: None }
    }

    pub fn from_context(ctx: Arc<ApplicationContext>) -> Self {
        Self { ctx: Some(ctx) }
    }

    pub fn index_path(&self, path: &str, opts: IndexOptions) -> Result<IndexResult, WalkerError> {
        let files = if let Some(explicit) = &opts.explicit_files {
            explicit
                .iter()
                .cloned()
                .map(|f_path| FileRecord {
                    path: f_path,
                    size: 0,
                    mtime: 0,
                    hash: String::new(),
                    language: None,
                })
                .collect()
        } else {
            let scan_opts = ScanOptions::new(path);
            walk_path(&scan_opts)?
        };

        let mut chunks = Vec::new();

        for file in &files {
            let lang = detect_language(&file.path);
            if lang.is_none() {
                continue;
            }
            let full_path = std::path::PathBuf::from(path).join(&file.path);
            let text = std::fs::read_to_string(&full_path).unwrap_or_default();
            let generated = chunk_file_contents(
                &file.path,
                &text,
                lang,
                None,
                &opts.chunking,
            );

            for chunk in generated {
                let payload = crate::application::protocol::ChunkEventPayload::from(chunk.clone());
                crate::infra::jsonl::write_chunk_event(None, &payload);
                chunks.push(chunk);
            }
        }

        Ok(IndexResult { chunks, files })
    }

    pub fn index_path_parallel(
        &self,
        path: &str,
        opts: IndexOptions,
        job_id: Option<String>,
    ) -> Result<IndexResult, WalkerError> {
        let files = if let Some(explicit) = &opts.explicit_files {
            explicit
                .iter()
                .cloned()
                .map(|f_path| FileRecord {
                    path: f_path,
                    size: 0,
                    mtime: 0,
                    hash: String::new(),
                    language: None,
                })
                .collect()
        } else {
            let scan_opts = ScanOptions::new(path);
            walk_path(&scan_opts)?
        };

        let pool = if let Some(ctx) = &self.ctx {
            ctx.parser_pool.clone()
        } else {
            Arc::new(ParserPool::new())
        };

        let bp_monitor: Option<Arc<BackpressureMonitor>> = opts
            .backpressure
            .as_ref()
            .map(|config| {
                Arc::new(
                    BackpressureMonitor::new(config.clone(), 0, job_id.clone())
                        .expect(
                            "BackpressureConfig inválida: max_queue_size mínimo é \
                             MIN_BACKPRESSURE_QUEUE_SIZE=10. Verifique a configuração \
                             do indexer no payload do comando.",
                        ),
                )
            });

        // Store monitor in global map if we have context and job_id
        if let (Some(ctx), Some(jid), Some(monitor)) =
            (&self.ctx, job_id.as_ref(), bp_monitor.as_ref())
        {
            ctx.backpressure_monitors
                .insert(jid.to_string(), Arc::clone(monitor));
        }

        let (tx, rx) = mpsc::channel();
        let base_path = Arc::new(std::path::PathBuf::from(path));
        let monitor_clone = bp_monitor.clone();

        files.par_iter().enumerate().with_max_len(1).for_each_with(
            (tx.clone(), monitor_clone),
            |(sender, monitor), (_idx, file)| {
                let lang = detect_language(&file.path);
                if lang.is_none() {
                    return;
                }
                let adapter_opt = lang.as_ref().and_then(|language| pool.get(language));
                let full_path = base_path.join(&file.path);
                let text = std::fs::read_to_string(&full_path).unwrap_or_default();

                let (parsed_text, symbols) = match &adapter_opt {
                    Some(adapter) => match adapter.parse_source(&text) {
                        Ok(mut parsed) => {
                            parsed.path = file.path.clone();
                            let symbols = adapter.extract_symbols(&parsed).ok();

                            if opts.extract_imports {
                                if let Ok(imports) = adapter.extract_imports(&parsed) {
                                    let lang_for_norm = lang.clone().unwrap_or_default();
                                    for raw_edge in imports {
                                        let normalized =
                                            normalize_import(&raw_edge, &lang_for_norm);
                                        crate::infra::jsonl::write_import_event(
                                            job_id.clone(),
                                            &normalized,
                                        );
                                    }
                                }
                            }

                            if opts.extract_calls {
                                if let Ok(calls) = adapter.extract_calls(&parsed) {
                                    for edge in calls {
                                        crate::infra::jsonl::write_call_event(
                                            job_id.clone(),
                                            &edge,
                                        );
                                    }
                                }
                            }

                            (parsed.source.clone(), symbols)
                        }
                        Err(_) => (text.clone(), None),
                    },
                    None => (text.clone(), None),
                };

                let chunks_for_file = chunk_file_contents(
                    &file.path,
                    &parsed_text,
                    file.language.clone().or(lang),
                    symbols.as_ref(),
                    &opts.chunking,
                );

                for chunk in chunks_for_file {
                    let chunk_for_event = chunk.clone();
                    let _ = sender.send((chunk, file.clone()));
                    let payload =
                        crate::application::protocol::ChunkEventPayload::from(chunk_for_event);
                    if let Some(ref monitor) = monitor {
                        let _ = crate::infra::jsonl::emit_chunk_with_backpressure(
                            monitor,
                            job_id.clone(),
                            &payload,
                        );
                    } else {
                        crate::infra::jsonl::write_chunk_event(job_id.clone(), &payload);
                    }
                }
            },
        );

        drop(tx);

        let mut chunks = Vec::new();
        let mut files_out: HashMap<String, FileRecord> = HashMap::new();

        for (chunk, file) in rx {
            files_out.entry(file.path.clone()).or_insert(file);
            chunks.push(chunk);
        }

        chunks.sort_by(|a, b| {
            a.file_path
                .cmp(&b.file_path)
                .then(a.start_line.cmp(&b.start_line))
                .then(a.end_line.cmp(&b.end_line))
                .then(a.id.cmp(&b.id))
        });
        let mut files_out = files_out.into_values().collect::<Vec<_>>();
        files_out.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(IndexResult {
            chunks,
            files: files_out,
        })
    }
}

fn chunk_file_contents(
    file_path: &str,
    source: &str,
    language: Option<String>,
    symbols: Option<&Vec<Symbol>>,
    chunking_opts: &ChunkingOptions,
) -> Vec<Chunk> {
    let normalized_symbols = symbols.map(|items| {
        let mut filtered = items
            .iter()
            .filter(|symbol| is_chunk_boundary_symbol(symbol))
            .cloned()
            .collect::<Vec<_>>();

        if filtered.iter().any(has_zero_based_lines) {
            for symbol in &mut filtered {
                symbol.start_line = symbol.start_line.saturating_add(1);
                symbol.end_line = symbol.end_line.saturating_add(1);
            }
        }

        filtered
    });

    let base_chunker: Box<dyn ChunkStrategy> = match chunking_opts.strategy {
        ChunkingStrategy::SymbolBoundary => {
            Box::new(SymbolBoundaryChunker::new(chunking_opts.max_lines))
        }
        ChunkingStrategy::Semantic => Box::new(SemanticChunker::new(chunking_opts.max_lines)),
        ChunkingStrategy::LineLimited => Box::new(LineLimitedChunker::new(chunking_opts.max_lines)),
    };

    let chunks = match normalized_symbols.as_ref() {
        Some(syms) if !syms.is_empty() => {
            let mut chunker: Box<dyn ChunkStrategy> = base_chunker;

            if chunking_opts.overlap_lines > 0 {
                chunker = Box::new(OverlapChunker::new(chunker, chunking_opts.overlap_lines));
            }

            if chunking_opts.include_context {
                chunker = Box::new(ContextInjectionChunker::new(chunker));
            }

            chunker.chunk_file(file_path, source, Some(syms))
        }
        _ => {
            // For files without symbols, we can still apply size-limited chunking
            let mut chunker: Box<dyn ChunkStrategy> = base_chunker;

            if chunking_opts.overlap_lines > 0 {
                chunker = Box::new(OverlapChunker::new(chunker, chunking_opts.overlap_lines));
            }

            chunker.chunk_file(file_path, source, None)
        }
    };

    let chunks: Vec<Chunk> = chunks
        .into_iter()
        .map(|mut chunk| {
            chunk.language = language.clone().or(chunk.language.clone());
            chunk
        })
        .collect();

    // Apply token counting if requested and feature is enabled
    #[cfg(feature = "token_counting")]
    if chunking_opts.token_counting {
        use crate::application::chunking::apply_token_count;
        for chunk in &mut chunks {
            apply_token_count(chunk);
        }
    }

    chunks
}

fn is_chunk_boundary_symbol(symbol: &Symbol) -> bool {
    matches!(
        symbol.kind.to_lowercase().as_str(),
        "function"
            | "struct"
            | "enum"
            | "trait"
            | "impl"
            | "class"
            | "method"
            | "constructor"
            | "mod"
    )
}

fn has_zero_based_lines(symbol: &Symbol) -> bool {
    symbol.start_line == 0 || symbol.end_line == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_file(dir: &std::path::Path, relative: &str, content: &str) {
        let file_path = dir.join(relative);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(file_path, content).unwrap();
    }

    #[test]
    fn index_path_returns_files_and_chunks() {
        let dir = tempdir().unwrap();
        write_file(dir.path(), "lib.rs", "fn main() {}\n");

        let indexer = Indexer::new();
        let opts = IndexOptions {
            max_concurrency: 1,
            explicit_files: None,
            extract_imports: false,
            extract_calls: false,
            chunking: ChunkingOptions::default(),
            backpressure: None,
        };
        let result = indexer
            .index_path(dir.path().to_str().unwrap(), opts)
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.chunks.len(), 1);
        let file = &result.files[0];
        assert_eq!(file.path, "lib.rs");
        let chunk = &result.chunks[0];
        assert_eq!(chunk.file_path, "lib.rs");
        assert_eq!(chunk.language.as_deref(), Some("rust"));
        assert!(chunk.content.contains("fn main"));
    }

    #[test]
    fn index_path_parallel_skips_files_without_registered_parser() {
        let dir = tempdir().unwrap();
        write_file(dir.path(), "notes.txt", "alpha\nbeta\n");

        let indexer = Indexer::new();
        let opts = IndexOptions {
            max_concurrency: 2,
            explicit_files: None,
            extract_imports: false,
            extract_calls: false,
            chunking: ChunkingOptions::default(),
            backpressure: None,
        };
        let result = indexer
            .index_path_parallel(dir.path().to_str().unwrap(), opts, None)
            .unwrap();

        assert_eq!(result.files.len(), 0);
        assert_eq!(result.chunks.len(), 0);
    }

    #[cfg(feature = "parsing")]
    fn build_context() -> Arc<ApplicationContext> {
        let registry = Arc::new(crate::app::bootstrap::Registry::new());
        registry.register("rust", Arc::new(crate::adapters::rust::RustAdapter));
        registry.register(
            "typescript",
            Arc::new(crate::adapters::typescript::TypeScriptAdapter),
        );
        registry.register(
            "javascript",
            Arc::new(crate::adapters::typescript::TypeScriptAdapter),
        );
        registry.register("java", Arc::new(crate::adapters::java::JavaAdapter));

        let pool = ParserPool::new();
        pool.register("rust", Arc::new(crate::adapters::rust::RustAdapter));
        pool.register(
            "typescript",
            Arc::new(crate::adapters::typescript::TypeScriptAdapter),
        );
        pool.register(
            "javascript",
            Arc::new(crate::adapters::typescript::TypeScriptAdapter),
        );
        pool.register("java", Arc::new(crate::adapters::java::JavaAdapter));

        Arc::new(ApplicationContext {
            registry,
            parser_pool: Arc::new(pool),
            config: crate::app::bootstrap::Config::new(1, 10),
            metrics: None,
            logger: None,
            backpressure_monitors: dashmap::DashMap::new(),
        })
    }

    #[cfg(feature = "parsing")]
    #[test]
    fn index_path_parallel_uses_symbol_chunks_with_context_prefix() {
        let dir = tempdir().unwrap();
        write_file(
            dir.path(),
            "src/lib.rs",
            "use std::fmt;\n\npub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n",
        );

        let indexer = Indexer::from_context(build_context());
        let opts = IndexOptions {
            max_concurrency: 2,
            explicit_files: None,
            extract_imports: false,
            extract_calls: false,
            chunking: ChunkingOptions::default(),
            backpressure: None,
        };

        let result = indexer
            .index_path_parallel(dir.path().to_str().unwrap(), opts, None)
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert!(!result.chunks.is_empty());

        let chunk = result
            .chunks
            .iter()
            .find(|chunk| {
                chunk.chunk_kind.as_deref() == Some("Symbol")
                    && chunk.content.contains("pub fn add")
            })
            .expect("expected a symbol chunk for pub fn add");
        assert!(!chunk.symbol_ids.is_empty());
        assert!(chunk.content.starts_with("use std::fmt;\n\n"));
        assert_eq!(
            chunk
                .metadata
                .as_ref()
                .and_then(|meta| meta.get("has_context_prefix")),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(chunk.language.as_deref(), Some("rust"));
    }

    #[cfg(feature = "parsing")]
    #[test]
    fn index_path_parallel_groups_related_symbols_semantically() {
        let dir = tempdir().unwrap();
        write_file(
            dir.path(),
            "src/lib.rs",
            "pub struct UserService {\n    repo: Repo,\n}\n\nimpl UserService {\n    pub fn new() -> Self {\n        Self { repo: Repo::new() }\n    }\n\n    pub fn add(&self) {\n        println!(\"add\");\n    }\n}\n\nfn unrelated() {}\n",
        );

        let indexer = Indexer::from_context(build_context());
        let opts = IndexOptions {
            max_concurrency: 2,
            explicit_files: None,
            extract_imports: false,
            backpressure: None,
            extract_calls: false,
            chunking: ChunkingOptions::default(),
        };

        let result = indexer
            .index_path_parallel(dir.path().to_str().unwrap(), opts, None)
            .unwrap();

        assert_eq!(result.files.len(), 1);
        assert_eq!(result.chunks.len(), 2); // UserService+impl, unrelated

        let user_service_chunks: Vec<&Chunk> = result
            .chunks
            .iter()
            .filter(|chunk| chunk.content.contains("UserService"))
            .collect();
        assert_eq!(user_service_chunks.len(), 1);
        let chunk = user_service_chunks[0];
        assert!(chunk.content.contains("pub struct UserService"));
        assert!(chunk.content.contains("impl UserService"));
        assert!(chunk.content.contains("pub fn new"));
        assert!(chunk.content.contains("pub fn add"));
        assert!(!chunk.content.contains("unrelated"));
    }
}
