# Rust Indexer - Technical Guide for AI Agents

## Project Overview
A multi-language source code indexer built in Rust with Tree-sitter parsers. Extracts symbols, generates chunks, and indexes repositories for semantic search. Features incremental Git-based indexing and import/call graph extraction.

## Key Features
- Multi-language support: Rust, TypeScript/JavaScript, Java, Go
- Tree-sitter based parsing for accurate symbol extraction
- Incremental indexing via Git integration
- Import and call graph extraction with JSONL event emission
- Parallel processing with Rayon for high throughput
- JSONL protocol for communication with external callers
- Backpressure handling with pause/resume mechanism

## Architecture Overview
```
src/
├── adapters/          # Language adapters (Rust, TS, Java, Go)
├── application/       # Indexer service and business logic
├── domain/            # Domain types, parser, normalization
├── infra/             # ParserPool, walker, git, benchmarks, JSONL
├── app/               # Bootstrap, dependency injection, config
└── lib.rs
tests/
├── smoke_*.rs         # Smoke and integration tests
├── *_tests.rs         # Unit tests per component
└── integration_*.rs   # Integration tests
```

## Current Status
- **Branch**: `feature/import-call-graph/pipeline-integration` (current)
- **Progress**: Phase-3 integration COMPLETE
  - Import/call edge extraction integrated into indexer pipeline
  - All 4 language adapters updated with extract_imports/extract_calls methods
  - IndexOptions now includes extract_imports/extract_calls booleans
  - JSONL event emission helpers added (write_import_event, write_call_event)
  - CLI handler switched to use index_path_parallel for edge extraction
  - Smoke test validates import_edge and call_edge events in stdout
- **Tests**: 186 passing (156 unit + 30 integration/smoke), 0 compiler warnings
- **Pending**: 
  - Backpressure-and-streaming subtask (max_queue_size, pause/resume events)
  - Phase-4 benchmarks & CI (100-1000 file benchmarks, CI smoke test)

## Main Commands for AI Agents

### Testing
```bash
# Run all tests (requires parsing feature)
make test

# Run only unit tests
make unit

# Run only integration tests
make integration

# Run only smoke tests
make smoke

# Run tests with verbose output
make test  # or cargo test --features parsing -- --nocapture
```

### Benchmarks
```bash
# Run all benchmarks with output
make bench

# Serial vs parallel comparison (200 files)
make bench-serial-parallel

# Throughput benchmarks (50, 100, 200 files)
make bench-throughput

# Large scale benchmarks (100, 500 files)
make bench-scale

# Full benchmark report
make bench-full
```

### Code Quality
```bash
# Check compilation without running tests
make check

# Format code
make format

# Run clippy (warnings as errors)
make lint

# Clean build artifacts
make clean
```

### Specific Test Scenarios
```bash
# Run incremental indexing smoke test
cargo test --test smoke_incremental_git -- --nocapture --features parsing

# Run multi-language smoke test
cargo test --test smoke_multi_lang -- --nocapture --features parsing

# Run import/call edge emission smoke test
cargo test --test cli_smoke -- --nocapture --features parsing
```

## Important Files for Context

### Core Implementation
- `src/adapters/mod.rs` - LanguageAdapter trait with extract_imports/extract_calls
- `src/adapters/*.rs` - Language-specific adapter implementations
- `src/application/indexer.rs` - Indexer pipeline with edge extraction logic
- `src/infra/jsonl.rs` - JSONL event emission helpers
- `src/domain/normalize.rs` - Import edge normalization

### Configuration & Protocol
- `doc/protocol.md` - JSONL protocol specification (events, commands, backpressure)
- `doc/indexer_spec.md` - Incremental indexing specification
- `doc/features/feature_import-call-graph.md` - Feature details and acceptance criteria
- `doc/architecture/bootstrap.md` - Dependency injection and application context

### Entry Points
- `src/main.rs` - Application entry point
- `src/cli/mod.rs` - CLI command handling and JSONL protocol
- `src/bin.rs` - Binary interface

## Development Workflow
1. Create feature branch from current integration branch
2. Implement changes following existing patterns
3. Run `make test` frequently to ensure no regressions
4. For UI-related changes, run smoke tests to validate end-to-end flow
5. For performance changes, run benchmarks to measure impact
6. Update documentation as needed in `/workspace/doc/`

## Current Focus Areas
Based on the feature doc, the immediate next tasks are:
1. **Backpressure-and-streaming**: Implement max_queue_size and pause/resume events
2. **Phase-4 benchmarks & CI**: Add performance benchmarks and CI smoke test

## Notes for AI Agents
- The project uses Rust 2021 edition
- Tree-sitter parsing is optional via `--features parsing` flag
- Tests requiring parsing must be run with the parsing feature
- JSONL is the primary communication protocol over stdio
- All events follow the protocol_version "1.0.0" format
- Backpressure mechanism uses pause/resume rather than explicit ACK in V1
- Import edges are normalized; call edges are emitted as-is (not normalized)
- CLI handlers default extract_imports/extract_calls to true for user convenience
- Tests set these to false to maintain backward compatibility
