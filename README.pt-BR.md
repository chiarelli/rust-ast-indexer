# Rust Indexer

> **Indexação de código-fonte rápida e adaptável a linguagens.** Construído sobre Rayon para vazão paralela e uma camada de adapters Tree-sitter plugável que torna adicionar uma nova linguagem questão de um módulo — não de uma reescrita.

[![CI](https://github.com/chiarelli/rust-ast-indexer/actions/workflows/ci.yml/badge.svg)](https://github.com/chiarelli/rust-ast-indexer/actions/workflows/ci.yml)

Um indexador de código-fonte multi-linguagem escrito em Rust, com parsing via Tree-sitter. Extrai símbolos, gera chunks e indexa repositórios para busca semântica — com indexação incremental baseada em Git e extração de grafos de importação/chamadas.

> **Read in English:** [README.md](README.md)

## Funcionalidades

- **Suporte multi-linguagem** — Rust, TypeScript, JavaScript, Python, Java, Go
- **Parsing via Tree-sitter** para extração precisa de símbolos
- **Indexação incremental** via integração com Git (ranges de diff, arquivos rastreados)
- **Extração de grafos de importação e chamadas** com emissão de eventos JSONL
- **Processamento paralelo** com Rayon para alta vazão
- **Protocolo JSONL** sobre stdio para comunicação com callers externos
- **Tratamento de backpressure** com mecanismo de pause/resume
- **Adapter MCP-compatível** via stdio

## Linguagens Suportadas

| Linguagem   | Parser                   | Símbolos Suportados                                           |
|-------------|--------------------------|---------------------------------------------------------------|
| Rust        | tree-sitter-rust         | fn, struct, enum, impl, trait, mod, use, const, static         |
| TypeScript  | tree-sitter-typescript   | function, class, method, interface, enum, type, import, export, variable |
| JavaScript  | tree-sitter-javascript   | function, class, method, import, export, variable              |
| Python      | tree-sitter-python       | function, async function, class, variable, import, decorated   |
| Java        | tree-sitter-java         | class, method, enum, interface, constructor, field, import     |
| Go          | tree-sitter-go           | function, struct, interface, method, import                    |

> **Nota:** TypeScript usa a grammar nativa `tree-sitter-typescript` — suporte completo a TSX, interfaces, type aliases e imports no estilo ES module. Python extrai funções síncronas e assíncronas, classes decoradas, e variáveis de módulo.

## Pré-requisitos

- Rust (stable)
- Cargo
- GNU Make

## Começando

```bash
# Build (binário release em target/release/rust_indexer)
make build

# Rodar todos os testes (unit + integração)
make test

# Rodar apenas testes unitários
make unit

# Rodar apenas testes de integração
make integration

# Rodar apenas smoke tests
make smoke
```

Ou use o Cargo diretamente:

```bash
cd rust_indexer
cargo test --features parsing
```

## Uso

O binário se comunica via **JSONL** (JSON delimitado por nova linha) sobre stdio. Na inicialização emite um evento `capabilities` e depois processa comandos enviados pelo caller.

### Indexar um caminho

```json
{"protocol_version":"1.0.0","type":"command","command":"index_path","seq":2,"job_id":"job-123","payload":{"path":"/proj","options":{"max_concurrency":8,"chunk_lines":200,"backpressure":{"max_queue_size":500,"ack_required":false}}}}
```

### Indexação incremental (modo Git)

```json
{"protocol_version":"1.0.0","type":"command","command":"incremental_index","seq":5,"job_id":"job-124","payload":{"path":"/proj","use_git":true,"git_range":{"from":"HEAD~1","to":"HEAD"},"options":{"max_concurrency":4}}}
```

### Modo MCP

```bash
rust_indexer --mcp
```

Veja [doc/protocol.md](rust_indexer/doc/protocol.md) para a especificação completa do protocolo (comandos, eventos, backpressure).

## Backpressure

O indexer aplica backpressure à fila de saída para que um consumidor lento nunca
seja sobrecarregado. Quando a fila atinge `max_queue_size`, a engine emite um
evento `pause` e bloqueia a produção; o caller deve drenar a fila enviando
comandos `ack`, após o que um evento `resume` é emitido.

> **Importante:** `max_queue_size` tem mínimo de `10`. Valores abaixo disso são
> rejeitados com um erro claro `BACKPRESSURE_CONFIG`. Além disso, um caller que
> apenas lê a saída **sem** enviar `ack` deixará o job preso em `pause`.

Veja [Armadilhas e boas práticas de backpressure](rust_indexer/doc/protocol.md#armadilhas-e-boas-pr%C3%A1ticas-achados-de-teste) para os detalhes completos.

## Problemas conhecidos e armadilhas

Alguns comportamentos não óbvios foram descobertos ao testar o indexer de ponta
a ponta. Antes de integrar um caller, revise as armadilhas documentadas:

- **Backpressure é um protocolo de mão dupla** — o produtor pausa, o consumidor
  deve confirmar (`ack`) para retomar. Veja a [seção de backpressure](#backpressure).
- **O mínimo de `max_queue_size` é 10** — valores menores são rejeitados com um
  erro claro `BACKPRESSURE_CONFIG` em vez de falhar silenciosamente.
- **Eventos `file_listed` ignoram o backpressure** — apenas chunks (e imports/calls
  quando habilitados) passam pelo controle de backpressure.

Veja [doc/protocol.md](rust_indexer/doc/protocol.md) para a especificação completa
do protocolo e a [seção de armadilhas](rust_indexer/doc/protocol.md#armadilhas-e-boas-pr%C3%A1ticas-achados-de-teste)
para o detalhamento de cada problema.

## Comandos do Make

| Comando                    | Descrição                                     |
|----------------------------|-----------------------------------------------|
| `make build`               | Build do binário release                      |
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

## Performance

### Serial vs Paralelo (200 arquivos)

| Métrica | Serial   | Paralelo (Rayon) | Speedup |
|---------|----------|------------------|---------|
| Tempo   | 105.6 ms | 28.6 ms          | **3.69x** |

> **Nota:** Em ambientes com recursos limitados (1 CPU core / containers), o paralelo pode ser mais lento que o serial devido ao overhead de threads. Ajuste `MAX_CONCURRENCY=1` ou use `cargo test --lib` para pular benchmarks.

### Throughput (arquivos/s, símbolos/s) — 6 linguagens

| Arquivos | Arquivos/segundo | Símbolos/segundo | Tempo aprox. |
|----------|------------------|------------------|--------------|
| 50       | 6,949            | 25,813           | ~15 ms       |
| 100      | 7,068            | 25,938           | ~14 ms       |
| 200      | 6,606            | 24,212           | ~30 ms       |

**Destaques:**
- **3.69x de speedup** com Rayon sobre execução serial (em ambiente multicore)
- **Throughput consistente** entre 6K-7K arquivos/s sem degradação com volume
- **24K-26K símbolos/s** — Tree-sitter parsing eficiente em 6 linguagens simultaneamente
- **Escalabilidade linear** — paralelismo não introduz overhead significativo

## Arquitetura

```
rust_indexer/
├── src/
│   ├── adapters/          # Language adapters (Rust, TS, Python, Java, Go)
│   ├── application/       # Serviço indexer e lógica de negócio
│   ├── domain/            # Tipos de domínio, parser, normalização
│   ├── infra/             # ParserPool, walker, git, benchmarks, JSONL
│   ├── app/               # Bootstrap, injeção de dependência, config
│   ├── cli/               # Tratamento de comandos CLI e protocolo JSONL
│   └── lib.rs
├── tests/                 # Testes unitários, de integração e smoke
├── doc/                   # Documentação e especificações de features
└── examples/              # Exemplos de uso (Node.js, Docker)
```

## Exemplos

- [Exemplo Node.js](rust_indexer/examples/nodejs/) — uso básico do JSONL via stdio
- [Exemplo Docker](rust_indexer/examples/docker/) — rodar rust_indexer em um container

## Documentação

- [Especificação do protocolo](rust_indexer/doc/protocol.md) — protocolo JSONL, comandos, eventos, backpressure
- [Especificação do indexer](rust_indexer/doc/indexer_spec.md) — indexação incremental
- [Chunking](rust_indexer/doc/chunking.md) — estratégias de chunking
- [Arquitetura](rust_indexer/doc/architecture/bootstrap.md) — injeção de dependência e contexto da aplicação

## Contribuindo

1. Crie uma branch de feature a partir da branch de integração atual
2. Implemente as mudanças seguindo os padrões existentes
3. Rode `make test` com frequência para garantir que não há regressões
4. Para mudanças de performance, rode benchmarks para medir o impacto
5. Atualize a documentação conforme necessário em `doc/`

## Licença

Este projeto é licenciado sob a [Licença MIT](LICENSE).
