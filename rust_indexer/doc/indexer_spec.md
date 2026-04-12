# Indexer specification — Git-based incremental indexing

This document describes the behavior of the `incremental_index` command and the Git integration used by the indexer (`crate::infra::git`). It specifies payload fields, expected semantics, events emitted by the CLI, error handling, and examples.

## Purpose

Enable incremental indexing driven by Git metadata so callers can request indexing only for files tracked by Git or for files changed between two refs. This reduces work and enables fast CI/agent workflows.

## Command: `incremental_index`

The CLI accepts a JSONL `Command` whose `command` field equals `incremental_index`. The `payload` object supports the following fields:

- `path` (string, required): repository/workspace path where the indexer should run.
- `use_git` (bool, optional, default: false): if true, the CLI will consult Git to discover the set of files to index instead of scanning the filesystem.
- `git_range` (object, optional): when present and `use_git=true`, it should contain `from` and `to` strings (refs / tags / commits) describing the git range to diff. If both `from` and `to` are present, the indexer will index files reported by `git diff --name-only <from> <to>`.
- `files` (array[string], optional): explicit list of file paths to index. Used when `use_git=false` or as an explicit override.
- `options` (object, optional): indexer options, such as `max_concurrency`, `extract_imports`, `extract_calls`, and `chunking`.

Example payloads:

- Full tracked files:

```json
{"path":"/repo", "use_git":true}
```

- Git diff between tags:

```json
{"path":"/repo", "use_git":true, "git_range": {"from":"v1","to":"HEAD"}}
```

- Explicit file list (no git):

```json
{"path":"/repo", "files":["src/lib.rs","README.md"], "options":{"max_concurrency":4}}
```

- With chunking options:

```json
{
  "path": "/repo",
  "use_git": true,
  "options": {
    "max_concurrency": 4,
    "chunking": {
      "strategy": "semantic",
      "max_lines": 200,
      "overlap_lines": 1,
      "include_context": true,
      "token_counting": false
    }
  }
}
```

## Behavior and semantics

1. Validate `payload.path` is present and non-empty. If missing, emit an `error` event with code `INVALID_PAYLOAD` and abort the job.
2. Determine file set:
   - If `use_git=true` and `git_range` contains both `from` and `to`, call `infra::git::get_git_diff_files(path, from, to)` to obtain the list of files changed between refs.
   - If `use_git=true` and `git_range` is absent or invalid, call `infra::git::emit_git_tracked_files(path)` to obtain all tracked files.
   - If `use_git=false` and the payload contains `files`, use that explicit list.
   - Otherwise, fall back to a full filesystem scan using `walk_path`.
3. If the `infra::git` call returns an error, emit an `error` event with code `GIT_ERROR`, include a helpful `message` describing the failure, then emit `job_completed` with `processed: 0` and `errors: 1`.
4. Construct an `IndexOptions` instance including `explicit_files: Option<Vec<String>>` when the file list was obtained from Git or from the payload.
5. Emit `job_started` event (with `job_id` when present) before indexing, and stream `file_listed` events for each discovered file (see `emit_file_listed_from_records` for the explicit-files case or `emit_file_listed_events` for filesystem scan).
6. Run indexer (`index_path_parallel`) with provided options. The indexer will emit `chunk_emitted` events as chunks are processed, and a final `job_completed` event with `processed` and `duration_ms`.

## Error handling

- Missing `git` binary or `not a git repository` errors are surfaced as `GIT_ERROR` with `recoverable:false` and will end the job early.
- Invalid `git_range` shapes fall back to tracked files, but if `emit_git_tracked_files` fails the job will error as above.
- Any `WalkerError` returned by the scanning/walking layer is reported as `WALKER_ERROR` and will result in `job_completed` with `errors:1`.

## Event stream contract

The command should produce a deterministic sequence of events on success:

1. `job_started` { job_id }
2. multiple `file_listed` events (stream)
3. multiple `chunk_emitted` events (stream)
4. `job_completed` { processed, duration_ms }

On Git errors, the sequence is:

1. `error` { code: "GIT_ERROR", message }
2. `job_completed` { processed: 0, errors: 1 }

## Notes and limitations

- File paths returned by the Git helper are relative to the repository root and are consumed as-is by the indexer. Callers should ensure the `path` argument is the repository root (or an appropriate subpath) to produce correct relative paths.
- The current implementation uses the system `git` binary. CI environments must provide `git` in PATH for `use_git=true` flows and for unit tests that exercise `infra::git`.
- For simplicity the first iteration uses placeholders for `FileRecord` metadata when explicit file lists are provided. The indexer computes or fills missing metadata during processing.

## Examples

- Request incremental indexing of files changed since tag `v1`:

```json
{
  "command": "incremental_index",
  "job_id": "job-123",
  "payload": { "path": ".", "use_git": true, "git_range": { "from": "v1", "to": "HEAD" } }
}
```

- Request indexing of tracked files only:

```json
{
  "command": "incremental_index",
  "payload": { "path": ".", "use_git": true }
}
```

## Next steps

- Add an integration smoke test (CI) that creates a temporary git repo, commits files, modifies them, and executes the binary with `incremental_index` payload to validate the end-to-end behavior. This will be implemented as the next task.

---

# Chunk Generation and `chunk_emitted` Schema

## Purpose

The indexer splits source files into semantically coherent chunks for consumption by LLMs or other tools. Chunks respect symbol boundaries, preserve context, and can be configured with different strategies.

## Chunk Schema

The `chunk_emitted` event contains a complete `Chunk` structure:

```json
{
  "type": "chunk_emitted",
  "chunk_id": "uuid-v4",
  "file_path": "src/services/user.rs",
  "language": "rust",
  "symbol_ids": ["src/services/user.rs::UserService", "src/services/user.rs::UserService::add"],
  "content": "pub struct UserService { ... }\n\nimpl UserService { pub fn add(...) { ... } }",
  "start_line": 12,
  "end_line": 45,
  "strategy": "symbol_boundary",
  "metadata": {
    "has_imports_prefix": true,
    "scope_chain": ["services", "user", "UserService"],
    "line_count": 180,
    "token_count": 512,
    "split_from_symbol": "UserService::add",
    "previous_chunk_id": "chk-...",
    "next_chunk_id": "chk-..."
  }
}
```

### Chunk Fields

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Always "chunk_emitted" |
| `chunk_id` | string | Unique identifier for the chunk |
| `file_path` | string | Relative path to the source file |
| `language` | string | Programming language (rust, typescript, javascript, java) |
| `symbol_ids` | array[string] | IDs of symbols contained in this chunk |
| `content` | string | Textual content of the chunk |
| `start_line` | number | 1-based line number where chunk starts |
| `end_line` | number | 1-based line number where chunk ends |
| `strategy` | string | Chunking strategy used |
| `metadata` | object | Extended metadata (see below) |

### Metadata Fields

| Field | Type | Description |
|-------|------|-------------|
| `has_imports_prefix` | boolean | Whether imports were injected as prefix |
| `scope_chain` | array[string] | Parent scopes (modules, classes) |
| `line_count` | number | Total lines in chunk |
| `token_count` | number | Approximate token count (if token_counting enabled) |
| `split_from_symbol` | string | Which symbol triggered a split |
| `previous_chunk_id` | string | ID of previous overlapping chunk |
| `next_chunk_id` | string | ID of next overlapping chunk |

## Chunking Strategies

Three strategies are supported via `ChunkingOptions.strategy`:

### 1. Symbol Boundary
- One chunk per symbol (function, class, struct, etc.)
- Preserves complete symbol definition
- Maximum chunk size enforced by `max_lines`

### 2. Semantic
- Groups related symbols together (impls with structs, methods with classes)
- Creates semantically coherent units
- Intelligent splitting when exceeding `max_lines`

### 3. Line Limited
- Simple line-based chunking
- Ignores symbol boundaries
- Useful for non-code files or fallback

## Chunking Configuration

The `chunking` object in `IndexOptions` allows full control over chunk generation:

```rust
pub struct ChunkingOptions {
    /// Chunking strategy to use
    pub strategy: ChunkingStrategy,
    /// Maximum lines per chunk (applies to size-limited strategies)
    pub max_lines: usize,
    /// Number of lines to overlap between consecutive chunks
    pub overlap_lines: usize,
    /// Whether to inject context (imports, parent scope) as prefix
    pub include_context: bool,
    /// Whether to count tokens in chunks (requires token_counting feature)
    pub token_counting: bool,
}
```

### Default Configuration

```json
{
  "strategy": "semantic",
  "max_lines": 200,
  "overlap_lines": 1,
  "include_context": true,
  "token_counting": false
}
```

## Chunking Pipeline

The chunking pipeline applies decorators in order:

1. **Base Strategy** (`SymbolBoundary`, `Semantic`, or `LineLimited`)
2. **Overlap Decorator** (if `overlap_lines > 0`) - adds `previous_chunk_id`/`next_chunk_id`
3. **Context Injection Decorator** (if `include_context: true`) - adds imports and scope chain as prefix
4. **Token Counting** (if `token_counting: true` and feature enabled) - adds `token_count` to metadata

## Integration with CLI

Both `index_path` and `incremental_index` commands support `chunking` options in the payload:

```json
{
  "command": "index_path",
  "payload": {
    "path": "/repo",
    "options": {
      "max_concurrency": 4,
      "chunking": {
        "strategy": "symbol_boundary",
        "max_lines": 200,
        "overlap_lines": 0,
        "include_context": true,
        "token_counting": false
      }
    }
  }
}
```

If `chunking` is not provided, defaults are used. The CLI automatically parses and validates chunking options.

## File Types and Fallback

- **Supported languages** (Rust, TypeScript, JavaScript, Java): Use symbol-based chunking
- **Unsupported files** (config files, markdown, plain text): Use line-limited fallback
- **Empty files**: Single chunk with empty content
- **Files without symbols**: Line-limited chunking applied

## Performance Considerations

- Chunking adds minimal overhead (< 5% of total indexing time)
- Overlap creates additional metadata but no duplication of content
- Context injection reuses imports between chunks of same file
- Token counting only enabled when `token_counting: true` and feature compiled

---

# Tree-sitter Language Adapters

## Purpose

The `LanguageAdapter` trait provides a unified interface for parsing source code and extracting symbols across multiple languages. It enables compile-time language adapter registration via Cargo features.

## Adapter Lifecycle

Adapters are registered at application bootstrap and stored in both a `Registry` (for lookup by name) and a `ParserPool` (thread-safe concurrent access via `DashMap`).

### Registration

```rust
let pool = ParserPool::new();
pool.register("rust", Arc::new(RustAdapter));
pool.register("typescript", Arc::new(TypeScriptAdapter));
pool.register("javascript", Arc::new(TypeScriptAdapter));
pool.register("java", Arc::new(JavaAdapter));
```

At bootstrap (with `--features parsing`), all adapters are automatically registered via the `register_language_adapter!` macro.

## LanguageAdapter Trait

```rust
use crate::domain::parser::ParsedFile;
use crate::domain::types::Symbol;
use anyhow::Result;

pub trait LanguageAdapter: Send + Sync + 'static {
    /// Parse source code into an intermediate form.
    /// Returns the parsed representation with raw source string.
    fn parse_source(&self, source: &str) -> Result<ParsedFile>;

    /// Extract symbols from a previously parsed file.
    /// Symbols include functions, classes, structs, fields, etc.
    fn extract_symbols(&self, parsed: &ParsedFile) -> Result<Vec<Symbol>>;

    /// Clone the adapter into a box for ownership transfer.
    fn box_clone(&self) -> Box<dyn LanguageAdapter>;
}
```

### Return Types

**ParsedFile**:

```rust
pub struct ParsedFile {
    pub source: String,
    pub tree: tree_sitter::Tree,
    pub language: &'static tree_sitter::Language,
}
```

**Symbol**:

```rust
pub struct Symbol {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub scope: Option<String>,
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: Option<String>,
}
```

## Symbol Normalization

After extraction, symbols go through a normalization pipeline (`domain/normalize.rs`):

1. **Qualified Name Generation**  
   Combines scope + name to produce `module::Class::method`
2. **Overload Detection**  
   Detects overloaded symbols (same file+kind+name) and assigns `is_overloaded=true`
3. **Overload Index Assignment**  
   Overload indices are assigned by source order (sorted by `start_line`)
4. **Sorting**  
   Final result is sorted by `file_path`, then `start_line`

## Language Detection

File extension → language mapping (`detect_language()` in `indexer.rs`):

| Extension  | Language   |
|------------|------------|
| `.rs`      | rust       |
| `.ts`, `.tsx` | typescript |
| `.js`, `.jsx` | javascript |
| `.java`    | java       |

Unsupported extensions are skipped silently.

## Multi-Language Processing

The indexer uses Rayon for parallel processing of files:

```rust
let result = indexer.index_path_parallel(path, IndexOptions { max_concurrency: 4 }, None)?;
```

Each file is parsed concurrently. The `ParserPool` ensures thread-safe adapter access via `DashMap`.

## ParserPool

```rust
use rust_indexer::infra::parser_pool::ParserPool;

let pool = ParserPool::new();
pool.register("rust", Arc::new(RustAdapter));
let adapter = pool.get("rust"); // returns Option<Arc<dyn LanguageAdapter>>
```

`ParserPool` is `Clone` and `Send + Sync`. Cloned instances share the underlying `DashMap`.

## Performance

See [README.md](README.md#resultados-de-performance) for benchmark results.

**Summary:**

- **3.69× speedup** when processing 200 files with Rayon vs serial
- **6K-7K files/s** throughput across Rust, TypeScript, and Java
- **24K-26K symbols/s** extraction rate
- Linear scalability with no degradation at 200 files

## Error Handling

| Error Type          | Source                        | Recovery                      |
|---------------------|-------------------------------|-------------------------------|
| Parse failure       | `tree-sitter` invalid syntax  | File skipped, chunk still emitted with empty symbols |
| Missing language    | No adapter registered         | Chunk emitted without symbols |
| Missing source      | File not readable             | Empty string used as fallback |
