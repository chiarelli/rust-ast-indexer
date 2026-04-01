# Rust Codebase Indexing Engine — Specification

## Project Overview

### Description
A Rust-based indexing engine executed as a standalone binary over stdio. It scans repositories, parses source files with Tree-sitter, extracts symbols, builds import and call graphs, generates semantic chunks, and streams structured JSONL results to a Node.js caller. The engine is stateless: it does not persist results, generate embeddings, or decide reindex cadence.

### Technical design
- Primary integration: executable binary spawned by Node.js via `child_process`.
- Protocol: JSONL over stdio.
- Parsing: Tree-sitter grammars per language.
- Parallelism: Rayon for file-level parallel parsing and chunk generation.
- Hashing: chunk and file metadata include deterministic hashes for caller-side deduplication.

### Data structures
- `FileRecord { path, size, mtime, hash, language }`
- `Symbol { id, kind, name, qualified_name, file_path, range, signature, visibility, parent_symbol_id }`
- `Chunk { id, chunk_kind, file_path, language, symbol_id, start_line, end_line, text, chunk_md5, size }`
- `IndexJob { job_id, path, language_filters, ignore_patterns, options }`

### Algorithms
- Walk repository with ignore support.
- Detect language by extension and optional fallback heuristics.
- Parse files, extract symbols/imports/calls, then chunk.
- Stream events incrementally instead of accumulating full in-memory results.

### Open questions
- Nenhuma no momento.

### Assumptions
- O chamador gerencia persistência, embeddings e política de reindex.
- O binário poderá ser executado em ambientes com Git instalado quando `use_git=true`.

## Goals and Non-goals

### Description
Define claramente o que a engine faz e o que deliberadamente fica fora de escopo.

### Technical design
- Goals:
  - Indexar rapidamente múltiplas linguagens suportadas.
  - Emitir saída estruturada e estável para consumo por Node.js.
  - Gerar símbolos, import graph, call graph e chunks semânticos.
  - Suportar processamento incremental quando o caller fornecer arquivos ou solicitar diff via Git.
- Non-goals:
  - Não armazenar dados internamente.
  - Não gerar embeddings.
  - Não executar busca semântica.
  - Não decidir automaticamente quando reindexar.

### Data structures
- `GoalSet { goals, non_goals }`

### Algorithms
- Aplicar validação de escopo em comandos recebidos.
- Rejeitar requests fora do contrato do protocolo.

### Open questions
- Nenhuma no momento.

### Assumptions
- O chamador aceita responsabilidade por armazenamento e orquestração.

## System Architecture

### Description
Arquitetura DDD-lite com separação entre domínio, aplicação, infraestrutura e CLI/binário.

### Technical design
- `domain/`: tipos centrais, contratos, regras de extração.
- `application/`: casos de uso (`index_path`, `list_languages`, `incremental_index`, `status`, `resume`).
- `infra/`: filesystem walking, parser pool, integração shell git, IO JSONL.
- `cli/`: loop stdio, parsing de comandos e streaming de eventos.
- Entrega principal: binário executável. Estrutura interna pode continuar modularizada como crate para testabilidade.
- Compatibilidade MCP: protocolo modelado para ser facilmente adaptável a MCP stdio no futuro.

### Data structures
- `JobStatus { job_id, state, processed_files, total_files, queued_events, is_paused, errors }`
- `Capabilities { version, protocol_version, languages, features }`

### Algorithms
- Startup emite `capabilities`.
- Loop principal lê JSONL, valida, despacha comando e transmite eventos.
- Operações longas executam por job identificado por `job_id`.

### Open questions
- Nenhuma no momento.

### Assumptions
- O chamador é um processo Node.js controlando o ciclo de vida do binário.

## Repository Scanning

### Description
A engine scans repositories and returns a curated `Vec<FileRecord>` reflecting caller-provided include/ignore patterns plus file metadata useful for downstream chunking.

### Technical design
- Uses `walkdir::WalkDir` configured via `ScanOptions` capturing include/ignore glob patterns and link behavior.
- Builds `GlobSet`s from caller-provided patterns (via `globset`) with fallback to global wildcard when no include patterns are supplied.
- Computes per-file metadata (`size`, `mtime`, `blake3` hash, language hint) so downstream components can deduplicate or schedule work without re-reading content.
- Supports `list_files`, `dry_run`, and `incremental_index` purely with metadata emission.
- Optionally honors `.gitignore`/`.crushignore` when the caller preloads them into ignore patterns, deferring auto-detection for a future iteration.

### Data structures
- `ScanRequest { path, include_patterns, ignore_patterns, use_git, git_range }`
- `ScanOptions { path: PathBuf, include_patterns: Vec<String>, ignore_patterns: Vec<String>, follow_links: bool }`
- `WalkerError { Glob(globset::Error) }`
- `FileRecord { path, size, mtime, hash, language }`

### Algorithms
- Build glob sets for include and ignore patterns (empty set means all files allowed).
- Walk directories recursively (optionally following symlinks) and skip non-files early.
- Normalize relative paths and apply ignore filters before include filters (ignores take priority).
- Gather metadata: file size, mtime, hash, language hint, then sort results deterministically.
- Skip files whose metadata or hash cannot be computed instead of failing the job.

### Open questions
- Future enhancement: load `.gitignore`/`.crushignore` automatically when requested.

### Assumptions
- Repositories are structured for recursive scanning.
- Caller provides ignore/ include patterns (including .gitignore contents if desired).
- Patterns follow `globset` semantics.

## Parallel Processing (Rayon Strategy)

### Description
O engine usa Rayon para maximizar throughput em máquinas multicore, mantendo isolamento de estado e previsibilidade operacional.

### Technical design
- `rayon::ThreadPoolBuilder` com default em `num_cpus::get()`.
- `max_concurrency` configurável por comando.
- `par_iter` por arquivo; operações intra-arquivo permanecem sequenciais para simplificar o uso do parser.
- Pool de parsers por thread para evitar compartilhamento mutável.
- Semáforo interno para evitar oversubscription quando caller pedir concorrência acima do hardware.
- Módulos seguem SOLID: responsabilidades pequenas, traits nas bordas, dependências invertidas.

### Data structures
- `ThreadPoolConfig { max_threads }`
- `ParserPool { per_thread_parser }`
- `BackpressureConfig { max_queue_size, pause_on_full, cancel_support }`

### Algorithms
- Distribuir arquivos entre workers Rayon.
- Cada worker obtém parser local ao thread.
- Resultados são emitidos incrementalmente em ordem de conclusão, não de descoberta.
- Quando a fila de saída enche, job entra em pausa até `resume`.

### Open questions
- Nenhuma no momento.

### Assumptions
- Ganho principal virá do paralelismo por arquivo, não por nó AST.

## AST Extraction (Tree-sitter)

### Description
A extração AST é a base para símbolos, imports, calls e chunking semântico.

### Technical design
- Cada linguagem suportada terá grammar Tree-sitter específica.
- A engine expõe apenas uma representação normalizada; detalhes específicos ficam encapsulados em adaptadores por linguagem.
- Parse errors parciais não cancelam o job inteiro; o arquivo pode produzir erro recoverable.

### Data structures
- `ParsedFile { language, root_kind, diagnostics }`
- `SourceRange { start_line, start_col, end_line, end_col }`
- `LanguageAdapter { parse, extract_symbols, extract_imports, extract_calls }`

### Algorithms
- Detectar linguagem.
- Parsear conteúdo em AST.
- Navegar na AST por queries/visita estruturada por linguagem.
- Produzir árvore intermediária normalizada para camadas superiores.

### Open questions
- Nenhuma no momento.

### Assumptions
- V1 cobre linguagens iniciais: Rust, Go, Python, TypeScript/JavaScript e Java.

## Symbol Extraction

### Description
A engine extrai símbolos de alto valor para navegação, chunking e grafos.

### Technical design
- V1 extrai: `module`, `class`, `struct`, `interface`, `enum`, `function`, `method`, `trait`, `type_alias`, `const`.
- Estrutura normalizada por linguagem.
- Símbolos carregam nome qualificado, assinatura, escopo e visibilidade quando disponível.

### Data structures
- `Symbol { id, kind, name, qualified_name, file_path, range, signature, visibility, parent_symbol_id }`
- `Visibility = Public | Private | Protected | Internal | Unknown`

### Algorithms
- Percorrer nós AST relevantes por linguagem.
- Resolver hierarquia pai-filho para composição de `qualified_name`.
- Produzir `symbol_id` determinístico a partir de caminho + nome qualificado + range.

### Open questions
- Nenhuma no momento.

### Assumptions
- Símbolos locais muito efêmeros (ex.: variáveis locais) ficam fora da V1.

## Import Graph

### Description
O import graph registra dependências de arquivo e, quando possível, de símbolos importados.

### Technical design
- Escopo principal: arquivo → módulo importado.
- Quando a linguagem permitir, incluir símbolo importado, alias e tipo de import.
- Resolver parcialmente; ambiguidades marcam `resolved=false`.

### Data structures
- `ImportEdge { from_file, to_module, imported_symbol, alias, import_kind, location, resolved }`
- `ImportKind = Named | Default | Wildcard | SideEffect | Relative | Unknown`

### Algorithms
- Extrair imports pela AST.
- Normalizar paths/módulos por linguagem.
- Registrar alias e imports relativos sem exigir resolução total para artefatos externos.

### Open questions
- Nenhuma no momento.

### Assumptions
- V1 prioriza fidelidade estrutural sobre resolução completa de módulos externos.

## Call Graph

### Description
O call graph registra chamadas diretas entre funções/símbolos detectáveis estaticamente.

### Technical design
- Foco em chamadas AST-based diretas.
- Chamadas dinâmicas/indiretas são mantidas como `resolved=false` ou `call_kind=dynamic`.
- Relação principal: caller → callee.

### Data structures
- `CallEdge { caller_symbol_id, callee_name, callee_symbol_id, call_kind, location, resolved }`
- `CallKind = Direct | Method | Constructor | External | Dynamic | Unknown`

### Algorithms
- Identificar contexto do símbolo chamador.
- Extrair invocações por linguagem.
- Tentar resolver contra símbolos do mesmo arquivo/projeto; se falhar, preservar `callee_name`.

### Open questions
- Nenhuma no momento.

### Assumptions
- Dispatch dinâmico e reflexão não serão resolvidos completamente na V1.

## Chunk Generation Strategy

### Description
Chunks são a unidade de saída textual para indexação semântica no caller.

### Technical design
- Regra base:
  - se `file_lines < 200`: emitir chunk do arquivo inteiro + chunks por símbolo.
  - caso contrário: emitir apenas chunks por símbolo.
- Cada chunk inclui `chunk_md5`, `size`, `language`, `chunk_kind` e `symbol_id` opcional.
- Chunks são emitidos incrementalmente por JSONL.

### Data structures
- `Chunk { id, chunk_kind, file_path, language, symbol_id, start_line, end_line, text, chunk_md5, size }`
- `ChunkKind = FullFile | Symbol | Contextual`

### Algorithms
- Contar linhas do arquivo.
- Selecionar ranges dos símbolos.
- Gerar texto do chunk, calcular hash e tamanho.
- Emitir `chunk_emitted` por chunk.

### Open questions
- Nenhuma no momento.

### Assumptions
- O caller pode filtrar ou deduplicar chunks usando `chunk_md5`.

## Embedding Preparation

### Description
A engine não gera embeddings, mas produz metadados suficientes para o caller preparar embeddings.

### Technical design
- Fornecer `chunk_md5`, `size`, `file_path`, `language`, `symbol_id`, `chunk_kind`.
- Não chamar serviços externos nem modelos locais de embedding.

### Data structures
- `EmbeddingReadyChunk = Chunk + metadata`

### Algorithms
- Apenas enriquecer o chunk já gerado com metadados estáveis.

### Open questions
- Nenhuma no momento.

### Assumptions
- O pipeline de embeddings existe exclusivamente no caller.

## Incremental Indexing

### Description
O controle de reindex é do caller; a engine apenas executa indexação incremental quando instruída.

### Technical design
- `incremental_index` aceita:
  - lista explícita de arquivos,
  - ou `use_git=true` com `git_range`.
- A engine não armazena snapshot prévio.
- O caller decide política de reindex por commit, cron, webhook ou evento local.

### Data structures
- `IncrementalRequest { path, files, use_git, git_range, options }`

### Algorithms
- Se `files` vierem preenchidos, usar essa lista.
- Se `use_git=true`, obter arquivos via shell git.
- Indexar somente o conjunto resultante.

### Open questions
- Nenhuma no momento.

### Assumptions
- O caller poderá combinar hashes emitidos com seu storage para deduplicação.

## Database Schema

### Description
Não há banco interno. Ainda assim, a documentação sugere um schema opcional para o caller.

### Technical design
- Apenas schema de exemplo; não implementado no engine.
- Estruturas sugeridas: `files`, `symbols`, `import_edges`, `call_edges`, `chunks`, `embeddings`.

### Data structures
- `files(id, path, hash, size, language, indexed_at)`
- `symbols(id, file_id, kind, name, qualified_name, start_line, end_line, signature, visibility, parent_symbol_id)`
- `import_edges(id, file_id, to_module, imported_symbol, alias, import_kind, start_line)`
- `call_edges(id, caller_symbol_id, callee_symbol_id, callee_name, call_kind, call_line, resolved)`
- `chunks(id, file_id, symbol_id, chunk_kind, start_line, end_line, chunk_md5, size, text)`
- `embeddings(id, chunk_id, provider, model, vector_ref, created_at)`

### Algorithms
- O caller persiste eventos emitidos pela engine nas tabelas equivalentes.

### Open questions
- Nenhuma no momento.

### Assumptions
- O schema servirá apenas como referência, não como contrato obrigatório.

## MCP Tooling for Code Navigation

### Description
A engine será consumida por stdio binário normal, mas o formato será compatível com futura adaptação MCP.

### Technical design
- V1: JSONL próprio, simples e estável.
- Compatibilidade MCP: envelopes e capacidades mantidos próximos do estilo MCP para facilitar adapter stdio.
- Futuro: adapter HTTP/SSE ou MCP puro sem mudar o core do indexador.

### Data structures
- `Capabilities { features: ["jsonl", "incremental_index", "git_diff", "pause_resume"] }`

### Algorithms
- Expor `capabilities` suficientemente descritivo para um adapter traduzir comandos/eventos.

### Open questions
- Nenhuma no momento.

### Assumptions
- Compatibilidade MCP é meta de integração, não protocolo nativo obrigatório na V1.

## Performance Strategy

### Description
A performance prioriza throughput com uso controlado de CPU e memória.

### Technical design
- `max_concurrency` default = número de CPUs.
- `max_queue_size` default = 500 eventos.
- Linha JSON máxima = 1MB.
- Pausa automática quando fila de saída enche.
- Streaming contínuo para reduzir picos de memória.

### Data structures
- `PerformanceLimits { max_concurrency, max_queue_size, max_json_line_bytes }`

### Algorithms
- Aplicar limites antes de emitir eventos.
- Usar semáforo para evitar oversubscription.
- Emitir `job_progress` periodicamente para o caller monitorar jobs longos.

### Open questions
- Nenhuma no momento.

### Assumptions
- O caller consumirá stdout em streaming, sem bloquear indefinidamente.

## Error Handling

### Description
Erros devem ser estruturados, recoverable quando possível, e nunca derrubar o job inteiro sem necessidade.

### Technical design
- Códigos padronizados: `PARSER_FAIL`, `IO_ERR`, `INVALID_COMMAND`, `BACKPRESSURE`, `CANCELLED`, `INTERNAL_ERR`, `FILE_INVALID`.
- Arquivos binários/não-UTF8 geram `file_invalid` e o job continua.
- Comandos inválidos geram `error` estruturado.

### Data structures
- `ProtocolError { code, message, recoverable, file_path, detail }`

### Algorithms
- Validar comando antes de iniciar job.
- Isolar erro por arquivo sempre que possível.
- Continuar processamento após erros recoverable.

### Open questions
- Nenhuma no momento.

### Assumptions
- O caller tratará erros recoverable sem reiniciar necessariamente o processo.

## Testing Strategy

### Description
A base deve ser altamente testada, cobrindo cenários felizes, tristes, smoke e integração.

### Technical design
- Unit tests em `__testes__` por módulo.
- Integration/smoke tests em `__tests-it` na raiz.
- Testes no mesmo módulo com `#[cfg(test)]` para helpers privados.
- Pipeline CI: `cargo fmt`, `cargo clippy`, `cargo test`, coverage e benchmarks.

### Data structures
- `TestMatrix { module, happy_path, error_path, smoke, integration }`

### Algorithms
- Cobrir APIs públicas e helpers privados relevantes.
- Smoke tests executam o binário e validam `list_languages`, `index_path`, `job_completed`, `file_invalid`, `resume`.

### Test Failure Policy
- Falhas em testes não devem ser tratadas automaticamente apenas "fazendo o teste passar".
- Primeiro passo: investigar se a falha é uma regressão do código de produção (ex.: alteração recente que quebrou comportamento esperado).
  - Se for regressão, corrigir o código de produção de forma a restaurar o comportamento esperado e então reexecutar os testes.
- Se não for regressão, verificar se os testes são mutuamente conflitantes ou dependem de estado/ordem de execução instável.
  - Em caso de testes conflitantes (mutually exclusive) ou flaky, isolar causa raiz e refatorar os testes para torná-los determinísticos.
- Nunca remover testes existentes sem uma justificativa documentada que envolva remoção ou alteração explícita de funcionalidade; qualquer remoção/alteração de teste deve ser acompanhada por documentação atualizando a especificação e changelog.
- Quando for necessário alterar um teste (por exemplo, refletindo uma mudança de especificação), o ticket/commit deve mencionar a decisão e a razão, e incluir testes adicionais que cubram o novo comportamento.

### Open questions
- Nenhuma no momento.

### Assumptions
- Cobertura próxima de 100% é meta para módulos centrais.

## Deployment Model

### Description
O artefato principal é um binário executável distribuído ao projeto Node.js consumidor.

### Technical design
- Entrega: binário compilado por plataforma alvo.
- Integração: Node.js via `child_process.spawn`.
- Biblioteca interna pode existir apenas para organização e testes, mas não é o contrato principal.

### Data structures
- `BinaryDistribution { platform, arch, version }`

### Algorithms
- Build por target conforme pipeline do projeto principal.
- Startup emite `capabilities` para negotiation simples.

### Open questions
- Nenhuma no momento.

### Assumptions
- O runtime de produção permitirá spawn de processo filho.

## Future Extensions

### Description
Extensões futuras possíveis sem comprometer a simplicidade da V1.

### Technical design
- Novas linguagens Tree-sitter.
- Resolução mais profunda de símbolos/imports/calls.
- Adapter MCP nativo.
- API HTTP/SSE opcional.
- Benchmark suite por linguagem e tamanho de repositório.

### Data structures
- `ExtensionCandidate { name, impact, complexity }`

### Algorithms
- Priorizar extensões que não quebrem o protocolo JSONL existente.

### Open questions
- Nenhuma no momento.

### Assumptions
- Evoluções futuras devem preservar backward compatibility do protocolo sempre que possível.

## Annex A — Example SQL schema for caller (suggested)

-- files
CREATE TABLE files (
  id TEXT PRIMARY KEY,
  path TEXT NOT NULL,
  hash TEXT,
  size BIGINT,
  language TEXT,
  indexed_at TIMESTAMP WITH TIME ZONE DEFAULT now()
);

-- symbols
CREATE TABLE symbols (
  id TEXT PRIMARY KEY,
  file_id TEXT REFERENCES files(id),
  kind TEXT,
  name TEXT,
  qualified_name TEXT,
  start_line INT,
  end_line INT,
  signature TEXT,
  visibility TEXT,
  parent_symbol_id TEXT
);

-- import edges
CREATE TABLE import_edges (
  id TEXT PRIMARY KEY,
  file_id TEXT REFERENCES files(id),
  to_module TEXT,
  imported_symbol TEXT,
  alias TEXT,
  import_kind TEXT,
  start_line INT
);

-- call edges
CREATE TABLE call_edges (
  id TEXT PRIMARY KEY,
  caller_symbol_id TEXT,
  callee_symbol_id TEXT,
  callee_name TEXT,
  call_kind TEXT,
  call_line INT,
  resolved BOOLEAN DEFAULT false
);

-- chunks
CREATE TABLE chunks (
  id TEXT PRIMARY KEY,
  file_id TEXT REFERENCES files(id),
  symbol_id TEXT,
  chunk_kind TEXT,
  start_line INT,
  end_line INT,
  chunk_md5 TEXT,
  size INT,
  text TEXT
);

-- embeddings (example for caller storage)
CREATE TABLE embeddings (
  id TEXT PRIMARY KEY,
  chunk_id TEXT REFERENCES chunks(id),
  provider TEXT,
  model TEXT,
  vector_ref TEXT,
  created_at TIMESTAMP WITH TIME ZONE DEFAULT now()
);

-- Indexes (examples)
CREATE INDEX idx_files_path ON files(path);
CREATE INDEX idx_symbols_file ON symbols(file_id);
CREATE INDEX idx_chunks_md5 ON chunks(chunk_md5);

## Annex B — Language support matrix (V1)

The matrix below describes V1 support per language for Symbols, Imports and Calls extraction. "Partial" means structural extraction is supported but full resolution is not guaranteed in all cases.

| Language | Symbols (examples) | Imports | Calls |
|---|---:|:---:|:---:|
| Rust | mod, struct, enum, trait, fn, impl, const | Named, use (relative) — partial resolution | Direct calls, method calls, dynamic dispatch marked `dynamic` |
| Go | package, struct, interface, func, method, const, var | import paths, named imports — partial resolution | Direct calls, method calls, interface calls marked `dynamic` when unresolved |
| Python | module, class, def, async def, assignment (top-level) | import, from ... import, wildcard — aliases may be unresolved | Direct calls, attribute calls; dynamic imports/calls marked `dynamic` |

