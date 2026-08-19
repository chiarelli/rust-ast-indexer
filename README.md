# Rust Indexer

> **Fast, language-adaptive source indexing.** Built on Rayon for parallel throughput and a pluggable Tree-sitter adapter layer that makes adding a new language a matter of one module — not a rewrite.

[![CI](https://github.com/chiarelli/rust-ast-indexer/actions/workflows/ci.yml/badge.svg)](https://github.com/chiarelli/rust-ast-indexer/actions/workflows/ci.yml)

A multi-language source code indexer written in Rust, powered by Tree-sitter. It extracts symbols, generates chunks, and indexes repositories for semantic search — with incremental Git-based indexing and import/call graph extraction.

> **Leia em português:** [README.pt-BR.md](README.pt-BR.md)

## Features

- **Multi-language support** — Rust, TypeScript, JavaScript, Python, Java, Go
- **Tree-sitter based parsing** for accurate symbol extraction
- **Incremental indexing** via Git integration (diff ranges, tracked files)
- **Import & call graph extraction** with JSONL event emission
- **Parallel processing** with Rayon for high throughput
- **JSONL protocol** over stdio for communication with external callers
- **Backpressure handling** with pause/resume mechanism
- **MCP-compatible** stdio adapter

## Supported Languages

| Language   | Parser                   | Supported Symbols                                                                 |
|------------|--------------------------|-----------------------------------------------------------------------------------|
| Rust       | tree-sitter-rust         | fn, struct, enum, impl, trait, mod, use, const, static                            |
| TypeScript | tree-sitter-typescript   | function, class, method, interface, enum, type, import, export, variable          |
| JavaScript | tree-sitter-javascript   | function, class, method, import, export, variable                                 |
| Python     | tree-sitter-python       | function, async function, class, variable, import, decorated                      |
| Java       | tree-sitter-java         | class, method, enum, interface, constructor, field, import                        |
| Go         | tree-sitter-go           | function, struct, interface, method, import                                       |

> **Note:** TypeScript uses the native `tree-sitter-typescript` grammar — full TSX, interfaces, type aliases, and ES-module style imports. Python extracts synchronous and asynchronous functions, decorated classes, and module variables.

## Prerequisites

- Rust (stable)
- Cargo
- GNU Make

## Getting Started

```bash
# Build (release binary at target/release/rust_indexer)
make build

# Run all tests (unit + integration)
make test

# Run only unit tests
make unit

# Run only integration tests
make integration

# Run only smoke tests
make smoke
```

Or run Cargo directly:

```bash
cd rust_indexer
cargo test --features parsing
```

## Usage

The binary communicates over **JSONL** (newline-delimited JSON) on stdio. On startup it emits a `capabilities` event, then processes commands sent by the caller.

### Index a path

```json
{"protocol_version":"1.0.0","type":"command","command":"index_path","seq":2,"job_id":"job-123","payload":{"path":"/proj","options":{"max_concurrency":8,"chunk_lines":200,"backpressure":{"max_queue_size":500,"ack_required":false}}}}
```

### Incremental index (Git mode)

```json
{"protocol_version":"1.0.0","type":"command","command":"incremental_index","seq":5,"job_id":"job-124","payload":{"path":"/proj","use_git":true,"git_range":{"from":"HEAD~1","to":"HEAD"},"options":{"max_concurrency":4}}}
```

### MCP mode

```bash
rust_indexer --mcp
```

See [doc/protocol.md](rust_indexer/doc/protocol.md) for the full protocol specification (commands, events, backpressure).

## Make Targets

| Command                    | Description                                     |
|----------------------------|-------------------------------------------------|
| `make build`               | Build release binary                            |
| `make test`                | Run all tests (unit + integration)              |
| `make unit`                | Run only unit tests (`--lib`)                   |
| `make integration`         | Run only integration tests (`tests/*.rs`)       |
| `make smoke`               | Run only smoke tests (`tests/smoke_*.rs`)       |
| `make bench`               | Run all benchmarks with detailed output         |
| `make bench-serial-parallel` | Serial vs parallel (200 files)                |
| `make bench-throughput`    | Throughput at 50, 100, 200 files                |
| `make bench-scale`         | Large scale: 100 and 500 files                  |
| `make bench-full`          | Full benchmark report                           |
| `make clean`               | Clean build artifacts                           |
| `make check`               | Check compilation without running               |
| `make format`              | Format code (`cargo fmt`)                       |
| `make lint`                | Run clippy with warnings as errors              |

## Performance

### Serial vs Parallel (200 files)

| Metric | Serial  | Parallel (Rayon) | Speedup |
|--------|---------|------------------|---------|
| Time   | 105.6 ms | 28.6 ms         | **3.69x** |

> **Note:** In resource-constrained environments (1 CPU core / containers), parallel can be slower than serial due to thread overhead. Set `MAX_CONCURRENCY=1` or use `cargo test --lib` to skip benchmarks.

### Throughput (files/s, symbols/s) — 6 languages

| Files | Files/second | Symbols/second | Approx. time |
|-------|--------------|----------------|--------------|
| 50    | 6,949        | 25,813         | ~15 ms       |
| 100   | 7,068        | 25,938         | ~14 ms       |
| 200   | 6,606        | 24,212         | ~30 ms       |

**Highlights:**
- **3.69x speedup** with Rayon over serial execution (multicore environments)
- **Consistent throughput** of 6K-7K files/s with no degradation at volume
- **24K-26K symbols/s** — efficient Tree-sitter parsing across 6 languages
- **Linear scalability** — parallelism introduces no significant overhead

## Architecture

```
rust_indexer/
├── src/
│   ├── adapters/          # Language adapters (Rust, TS, Python, Java, Go)
│   ├── application/       # Indexer service and business logic
│   ├── domain/            # Domain types, parser, normalization
│   ├── infra/             # ParserPool, walker, git, benchmarks, JSONL
│   ├── app/               # Bootstrap, dependency injection, config
│   ├── cli/               # CLI command handling and JSONL protocol
│   └── lib.rs
├── tests/                 # Unit, integration, and smoke tests
├── doc/                   # Docs and feature specifications
└── examples/              # Usage examples (Node.js, Docker)
```

## Examples

- [Node.js example](rust_indexer/examples/nodejs/) — basic JSONL stdio usage
- [Docker example](rust_indexer/examples/docker/) — run rust_indexer in a container

## Documentation

- [Protocol specification](rust_indexer/doc/protocol.md) — JSONL protocol, commands, events, backpressure
- [Indexer specification](rust_indexer/doc/indexer_spec.md) — incremental indexing
- [Chunking](rust_indexer/doc/chunking.md) — chunking strategies
- [Architecture](rust_indexer/doc/architecture/bootstrap.md) — dependency injection and application context

## Contributing

1. Create a feature branch from the current integration branch
2. Implement changes following existing patterns
3. Run `make test` frequently to ensure no regressions
4. For performance changes, run benchmarks to measure impact
5. Update documentation as needed in `doc/`

## License

This project is licensed under the [MIT License](LICENSE).
