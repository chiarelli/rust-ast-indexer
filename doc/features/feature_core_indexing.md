# Feature: feature/core-indexing

## Objetivo
Entregar o núcleo executável do indexador: binário JSONL sobre stdio, dispatcher CLI, descoberta de arquivos, pipeline paralelo com Rayon, pool de parsers (placeholder) e emissão mínima de chunks.

## Fase 1 — scaffold & protocolo
Descrição: construir esqueleto do binário, garantir protocolo JSONL estável, implementar dispatcher de comandos (list_languages, index_path, dry_run, list_files, incremental_index -> use_git opcional), walker funcional, pipeline paralelo mínimo que produz e emite chunks (placeholders de conteúdo).

## Organização (feature → fases → tasks → atividades)

- feature/core-indexing
  - fase-1: scaffold & protocolo
    - task: infra/cli
      - atividade (commit): chore(cli): add JSONL stdio dispatcher
      - atividade (commit): feat(cli): implement list_languages capability event
      - atividade (commit): feat(cli): implement index_path dispatch and job threading
    - task: infra/walker
      - atividade: feat(walker): implement walkdir file discovery
      - atividade: test(walker): add unit tests for file filtering
    - task: application/indexer
      - atividade: feat(indexer): add rayon file-parallel pipeline scaffold
      - atividade: perf(indexer): add parser-pool placeholder and semaphore
    - task: chunking/protocol
      - atividade: feat(protocol): add chunk_emitted/job events schema
      - atividade: docs(protocol): add protocol examples in doc/protocol.md
    - task: infra/git
      - atividade: feat(git): add shell-git diff helper (use_git mode)
      - atividade: feat(git): integrate infra::git with CLI incremental_index and IndexOptions explicit_files (completed)
      - atividade: test(git): add unit tests for git helper and integration smoke for incremental_index (completed)
    - task: tests/ci
      - atividade: chore(ci): add GitHub Actions skeleton (fmt/clippy/test)
      - atividade: test(smoke): add smoke test for list_languages + index_path

Cada atividade corresponde a um commit com o título no formato: type(scope): short description
Exemplo: `feat(walker): implement walkdir file discovery`.
Branches por task: `feature/core-indexing/<task-name>` (ex.: `feature/core-indexing/infra-walker`).

Cada task DEVE incluir testes de unidade cobrindo cenários 'happy' e 'unhappy' (erro), e deve cobrir todas as funções/métodos envolvidos, incluindo helpers privados (via #[cfg(test)] no mesmo módulo).

Critérios de aceitação por atividade:
- Todos os testes unitários passam localmente (`cargo test`).
- `cargo clippy` deve passar sem warnings (usar `-D warnings` na CI).
- Cobertura de unidade para o módulo alvo é >= 90% (meta; documentar exceções).
- Commit está nomeado no formato `type(scope): short description`.
- Ao commitar: usar explicitamente `git add <file>` (não `git add .`).
- Testes de integração/smoke para a task rodando via binário devem existir quando aplicável.
- Documentação mínima (README ou comentário no módulo) explicando a função e como testar.


## Algoritmos (fase 1)

### CLI dispatcher
- Loop ler linhas de stdin (BufRead). Para cada linha parsear JSON em Command.
- Validar campos mínimos (`protocol_version`, `type==command`, `command`).
- Despachar por `command` para handlers (sincronizar jobs longos abrindo thread por `job_id`).
- Emitir eventos JSONL com `json!` para stdout via helper (serialize + println!).

### Walkdir file discovery (walker)
- Receber `ScanRequest{path, include_patterns?, ignore_patterns?}`.
- Usar walkdir::WalkDir para listar arquivos recursivamente.
- Aplicar filtros por extensão e `ignore_patterns` (glob-like).
- Para cada arquivo válido, calcular metadados: size, mtime, blake3 hash (opcional em dry_run pode omitir hash).
- Retornar Vec<FileRecord> ou emitir `file_listed` se em streaming.

### Pipeline paralelo (indexer)
- Receber lista de arquivos a indexar.
- Construir rayon::ThreadPool com `max_concurrency`.
- Criar ParserPool por thread (pre-warm Tree-sitter instances — placeholder inicial).
- Par_iter sobre arquivos: para cada arquivo -> parse (via ParserPool), extrair símbolos (adapter), gerar chunks.
- Por chunk produzido, enviar ao collector de saída (canal bounded).
- Collector escreve eventos JSONL para stdout (respeitando backpressure).

### ParserPool (concurrency hardening)
- Manter um parser por thread (ThreadLocal ou pool indexado por thread id).
- Evitar compartilhamento de Parser entre threads.
- Inicializar grammars reutilizáveis (lazy).
- Expor API: parse_source(language, &str) -> ParsedFile.

### Emissão de chunks e backpressure (v1 simples)
- Usar channel bounded (tokio::sync::mpsc or std::sync::mpsc with semaphore) com capacidade `max_queue_size`.
- Workers enviam eventos para o sender. Collector lê do receiver e faz println!.
- Quando channel está cheio, workers detectam e suspendem (parking ou await) — em v1 preferir pausar job e emitir `error{code:BACKPRESSURE, pause_required:true}`; esperar `resume` do caller.

### Shell-git diff helper
- Invocar `git -C <path> diff --name-only <from> <to>` e parsear linhas de saída em paths relativos.
- Validar saída vazia como sinal de "sem alterações".
- Em caso de erro no git, emitir `error{code:IO_ERR,...}` e retornar lista vazia.

## Mapping activities → commits (exemplo de mensagens)
- chore(cli): add JSONL stdio dispatcher
- feat(cli): implement list_languages capability event
- feat(walker): implement walkdir file discovery
- feat(indexer): add rayon file-parallel pipeline scaffold
- feat(git): add shell-git diff helper (use_git mode)
- chore(ci): add GitHub Actions skeleton (fmt/clippy/test)

## Próximas Features (placeholders)
- feature/tree-sitter-adapters
- feature/import-call-graph
- feature/backpressure-and-resume
- feature/chunking-heuristics
- feature/mcp-adapter
- feature/integration-tests-and-benchmarks
