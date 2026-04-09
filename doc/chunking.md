# Chunking heuristics

This document describes the chunk model and the current chunking strategies used by `rust_indexer`.

## Chunk model

A chunk is the unit emitted by the indexer pipeline and serialized in `chunk_emitted` events.

### Domain fields

- `id`: stable chunk identifier
- `file_path`: relative source file path
- `start_line` / `end_line`: 1-based inclusive line range
- `content`: primary textual content for indexing and downstream consumption
- `text`: compatibility alias for the same content
- `md5`: content hash used by event payloads and deduplication
- `size`: byte size of the chunk content
- `language`: optional detected language
- `symbol_id`: primary symbol associated with the chunk, when applicable
- `symbol_ids`: all symbol identifiers covered by the chunk
- `chunk_kind`: `FullFile`, `Symbol`, or `Contextual`
- `metadata`: extensible key/value bag for context and strategy-specific data

### Validation

The current validation rule is intentionally small:

- `start_line` and `end_line` must be at least 1
- `start_line` must be less than or equal to `end_line`

## Event schema

`chunk_emitted` events are JSONL events with a payload that contains:

- `chunk_id`
- `chunk_kind`
- `file`
- `language`
- `symbol_id`
- `start_line`
- `end_line`
- `text`
- `chunk_md5`
- `size`

The event payload is intentionally smaller than the domain `Chunk` model.

## Strategies

### Line-limited semantic chunker

`SemanticChunker` groups related symbols and uses a 200-line limit by default.

Behavior:

- groups symbols by semantic anchor when possible
- keeps structs/classes/traits aligned with related impl blocks
- uses scope information when adapters provide it
- falls back to a full-file chunk when no symbols are provided
- stores `max_line_limit` in metadata

Best suited for source files where related declarations should remain adjacent for downstream retrieval.

### Symbol boundary chunker

`SymbolBoundaryChunker` splits source code at symbol boundaries.

Behavior:

- emits one chunk per symbol when symbols are available
- falls back to a full-file chunk when no symbols are provided
- drops symbol chunks that exceed the configured line limit

Best suited for source files where exact symbol boundaries are important.

### Line-limited chunker

`LineLimitedChunker` groups adjacent symbols until the configured line limit would be exceeded.

Behavior:

- keeps adjacent symbols together when they fit the limit
- splits between symbols when the group would become too large
- preserves oversized symbols in their own chunk
- falls back to a full-file chunk when no symbols are available
- stores `max_line_limit` in metadata
- optionally adds `token_count` metadata when the `token_counting` feature is enabled

Best suited for balancing semantic grouping with deterministic size limits.

### Context injection decorator

`ContextInjectionChunker` wraps another chunker and prefixes emitted chunks with context.

Current context sources:

- leading imports
- scope chain information from symbols

It also updates:

- `content`
- `text`
- `md5`
- `size`
- `metadata`

The metadata currently includes:

- `has_context_prefix`
- `context_import_count`
- `context_scope_count`

### Overlap decorator

`OverlapChunker` wraps another chunker and expands each chunk to include neighboring line context.

Behavior:

- includes the previous chunk’s trailing lines when available
- includes the next chunk’s leading lines when available
- stores `previous_chunk_id` and `next_chunk_id` when those neighbors exist
- preserves inner chunk metadata and adds `overlap_lines`

Best suited for preserving boundary context without changing the underlying chunking strategy.

## Indexer integration

The indexer now uses the chunking abstractions when building emitted chunks. In the current wiring:

- symbol-aware files use the semantic strategy
- files with extracted symbols receive context injection
- unsupported or symbol-less files fall back to full-file chunks
- overlap is applied around emitted chunks to preserve boundary context

## Current status

Implemented:

- `Chunk` model expansion and validation
- symbol-boundary chunking
- line-limited chunking
- context injection decorator
- overlap decorator
- indexer integration with `chunk_emitted`
- smoke coverage for emitted chunk payloads

Planned next steps for later phases:

- CLI-exposed chunking configuration
- optional token counting feature enabled by `token_counting`
- fallback chunker for symbol-less files
- chunker benchmarks
- multi-strategy smoke test
