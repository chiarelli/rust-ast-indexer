# rust_indexer

Indexer de código-fonte com suporte multi-linguagem via Tree-sitter.

Extrai símbolos, gera chunks e indexa repositórios para busca semântica.
Arquitetura DDD-lite com módulos: domain, application, infra, cli.

## Funcionalidades

| Linguagem   | Parser                         | Símbolos Suportados                                           |
|-------------|--------------------------------|---------------------------------------------------------------|
| Rust        | tree-sitter-rust               | fn, struct, enum, impl, trait, mod, use, const, static         |
| TypeScript  | tree-sitter-typescript         | function, class, method, interface, enum, type, import, export, variable |
| JavaScript  | tree-sitter-javascript         | function, class, method, import, export, variable              |
| Python      | tree-sitter-python             | function, async function, class, variable, import, decorated   |
| Java        | tree-sitter-java               | class, method, enum, interface, constructor, field, import     |
| Go          | tree-sitter-go                 | function, struct, interface, method, import                    |

> **Nota:** TypeScript usa a grammar nativa `tree-sitter-typescript` — suporte completo a TSX, interfaces, type aliases e imports no estilo ES module. Python extrai funções síncronas e assíncronas, classes decoradas, e variáveis de módulo.

## Pré-requisitos

- Rust (stable)
- Cargo
- Make (GNU Make)

## Comandos do Make

O projeto inclui um Makefile com atalhos para os comandos mais comuns. Use `make help` para listar todos os targets.

| Comando                    | Descrição                                     |
|----------------------------|-----------------------------------------------|
| `make test`                | Todos os testes (unit + integração)           |
| `make unit`                | Só unitários (`--lib`)                        |
| `make integration`         | Só testes de integração (`tests/*.rs`)        |
| `make smoke`               | Só smoke tests (`tests/smoke_*.rs`)           |
| `make bench`               | Todos os benchmarks com saída detalhada       |
| `make bench-serial-parallel` | Serial vs paralelo (200 files)              |
| `make bench-throughput`    | Throughput com 50, 100, 200 arquivos          |
| `make bench-scale`         | Escala grande: 100 e 500 arquivos             |
| `make bench-full`          | Relatório completo de benchmarks              |
| `make clean`               | Limpa build artifacts                         |
| `make check`               | Verifica compilação sem executar              |
| `make format`              | Formata o código (`cargo fmt`)                |
| `make lint`                | Executa clippy com warnings como erro         |

## Executando testes

O comando completo é:

```bash
cd rust_indexer && cargo test --features parsing
```

Ou use `make test`, `make unit`, `make bench` — veja [Comandos do Make](#comandos-do-make).

## Resultados de Performance

### Serial vs Paralelo (200 arquivos)

| Métrica    | Serial   | Paralelo (Rayon) | Speedup |
|------------|----------|-------------------|---------|
| Tempo      | 105.6 ms | 28.6 ms           | **3.69x** |

> **Nota:** Em ambientes com recursos limitados (1 CPU core / containers), o paralelo pode ser mais lento que o serial devido ao overhead de threads. Ajuste `MAX_CONCURRENCY=1` no ambiente ou use `cargo test --lib` para pular benchmarks.

### Throughput (arquivos/s, símbolos/s) — 3 linguagens (Rust + TS + Go + Python + Java)

| Arquivos  | Arquivos/segundo | Símbolos/segundo | Tempo aprox. |
|-----------|-------------------|-------------------|--------------|
| 50        | 6,949             | 25,813            | ~15 ms       |
| 100       | 7,068             | 25,938            | ~14 ms       |
| 200       | 6,606             | 24,212            | ~30 ms       |

**Destaques:**
- **3.69x de speedup** com Rayon sobre execução serial (em ambiente multicore)
- **Throughput consistente** entre 6K-7K arquivos/s sem degradação com volume
- **24K-26K símbolos/s** — Tree-sitter parsing eficiente em 6 linguagens simultaneamente
- **Escalabilidade linear** — paralelismo não introduz overhead significativo

## Arquitetura

```
src/
├── adapters/          # Language adapters (Rust, TS, Python, Java, Go)
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
