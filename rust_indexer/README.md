# rust_indexer

Indexer de código-fonte com suporte multi-linguagem via Tree-sitter.

Extrai símbolos, gera chunks e indexa repositórios para busca semântica.
Arquitetura DDD-lite com módulos: domain, application, infra, cli.

## Funcionalidades

| Linguagem   | Parser                  | Símbolos Suportados                           |
|-------------|-------------------------|-----------------------------------------------|
| Rust        | tree-sitter-rust        | fn, struct, enum, impl, trait, mod, use, const, static |
| TypeScript  | tree-sitter-javascript  | function, class, method, import, export, var  |
| JavaScript  | tree-sitter-javascript  | function, class, method, import, export, var  |
| Java        | tree-sitter-java        | class, method, enum, interface, constructor, field, import |

## Pré-requisitos

- Rust (stable)
- Cargo

## Executando testes

### Testes unitários e de integração

```bash
cd rust_indexer && cargo test --features parsing
```

### Testes específicos por módulo

```bash
# Adaptação por linguagem (44 testes de adapters)
cargo test --features parsing adapters

# Parsing pool (9 testes de integração multi-linguagem)
cargo test --features parsing parser-pool

# Normalização de símbolos (23 testes)
cargo test --features parsing normalize

# Smoke tests multi-linguagem
cargo test --features parsing smoke_multi_lang
```

### Performance e Benchmarks

```bash
# Todos os benchmarks do ParserPool
cargo test --features parsing infra::benchmarks -- --nocapture

# Benchmark: 100 arquivos serial
cargo test --features parsing bench_index_100_files_all_languages -- --nocapture

# Benchmark: 500 arquivos com paralelismo Rayon
cargo test --features parsing bench_index_500_files_parallel -- --nocapture

# Comparação serial vs paralelo (200 arquivos)
cargo test --features parsing bench_serial_vs_parallel_comparison -- --nocapture

# Throughput com scaling (50, 100, 200 arquivos)
cargo test --features parsing bench_throughput_scales_with_file_count -- --nocapture
```

> **Nota:** A flag `--nocapture` é necessária para visualizar métricas de performance.

## Resultados de Performance

### Serial vs Paralelo (200 arquivos)

| Métrica    | Serial   | Paralelo (Rayon) | Speedup |
|------------|----------|-------------------|---------|
| Tempo      | 105.6 ms | 28.6 ms           | **3.69x** |

### Throughput (arquivos/s, símbolos/s)

| Arquivos  | Arquivos/segundo | Símbolos/segundo | Tempo aprox. |
|-----------|-------------------|-------------------|--------------|
| 50        | 6,949             | 25,813            | ~15 ms       |
| 100       | 7,068             | 25,938            | ~14 ms       |
| 200       | 6,606             | 24,212            | ~30 ms       |

**Destaques:**
- **3.69x de speedup** com Rayon sobre execução serial
- **Throughput consistente** entre 6K-7K arquivos/s sem degradação com volume
- **24K-26K símbolos/s** — Tree-sitter parsing eficiente em 3 linguagens simultaneamente
- **Escalabilidade linear** — paralelismo não introduz overhead significativo

## Arquitetura

```
src/
├── adapters/          # Language adapters (Rust, TS, Java)
├── application/       # Indexer service
├── domain/            # Domain types, parser, normalize
├── infra/             # ParserPool, walker, benchmarks
├── app/               # Bootstrap, config
└── lib.rs
tests/
├── smoke_multi_lang.rs  # Multi-language smoke tests
└── ...
```

## Estrutura de features

```bash
# Sem parsing (compila sem tree-sitter)
cargo test

# Com parsing (adapters habilitados)
cargo test --features parsing
```
