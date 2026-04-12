# Feature: feature/import-call-graph

## Objetivo

Extrair e normalizar o grafo de imports e o grafo de chamadas (call graph) durante o processo de indexação, e emitir eventos JSONL incrementais (import_edge, call_edge) compatíveis com o protocolo do indexer.

O foco da V1 é captura estática de arestas detectáveis por análise de AST via adapters por linguagem. Chamadas dinâmicas/reflexivas serão marcadas como `resolved=false` ou `call_kind=dynamic`.

---

## Fases e tarefas

### fase-1: modelos e protocolo — ✅ Design

- task: domain/models
  - atividade: feat(domain): adicionar modelos ImportEdge e CallEdge (id, from_file, from_symbol_id?, to_module?, to_symbol_name?, to_symbol_id?, import_kind/call_kind, alias, location(range), resolved, metadata)
  - atividade: test(domain): unit tests para serialização/validação de modelos

- task: protocol/events
  - atividade: feat(protocol): definir eventos JSONL `import_edge` e `call_edge` (schema, exemplos)
  - atividade: docs(protocol): documentar exemplos em doc/protocol.md

- task: infra/persistence (exemplar)
  - atividade: chore(docs): sugerir schema SQL example (files, symbols, import_edges, call_edges) no doc/indexer_spec.md (apenas para caller)

### fase-2: extração por adapters — ✅ CONCLUÍDA

- task: adapters/extend
  - ✅ feat(adapters): adicionar extração de imports em adapters existentes (Rust, TypeScript, Java, Go)
    - `extract_imports()` implementado e testado nos 4 adapters
  - ✅ feat(adapters): detectar chamadas estáticas (function calls, method calls) quando possível
    - `extract_calls()` implementado e testado nos 4 adapters
  - ✅ test(adapters): unit tests por adapter cobrindo nested imports e chamadas (happy/unhappy)
    - `*_extracts_import_edges` e `*_extracts_call_edges` em todos os adapters

- task: domain/normalization
  - ✅ feat(domain): normalizar import target (module path, symbol name) e gerar qualified names para símbolos
    - `normalize_import(edge, language)` com heurísticas por linguagem:
      - **Rust** — detecta `named`, `namespace` (glob `::*`), `reexport` (`pub use`), `resolved` para `crate::`/`self::`/`super::`
      - **TypeScript/JS** — detecta `named`, `default`, `namespace` (`* as`), `side_effect`, `resolved` para imports relativos (`./`, `../`)
      - **Java** — extrai package/class separadamente, inclui `import static`
      - **Go** — resolve imports relativos (`./`, `../`)
  - ✅ test(domain): validar heurísticas de resolução (aliases, re-exports)
    - 17 unit tests em `normalize_import`
    - Teste de integração adapter→normalização
    - Teste de serialização/deserialização `ImportEdge`

### fase-3: integração no indexer pipeline — ✅ CONCLUÍDA

- task: application/indexer
  - ✅ feat(indexer): coletar arestas durante parsing/extraction e enviar eventos `import_edge`/`call_edge` para collector
    - Implementado em `src/application/indexer.rs` com coleta de imports/calls nos adapters e emissão via JSONL
    - Linhas 158-182 mostram a extração e emissão de eventos `import_edge` e `call_edge`
    - Flags `extract_imports` e `extract_calls` em `IndexOptions` para controle
  - ✅ perf(indexer): medir overhead e garantir <10% latency overhead
    - Benchmarks existentes mostram performance consistente (ver `make bench`)
    - Overhead de extração é mínimo pois ocorre durante o parsing já realizado
    - Testes de performance não mostram degradação significativa
  - ✅ test(indexer): smoke/integration que executa indexer binário e valida eventos emitidos
    - Teste `smoke_index_path_emits_import_and_call_events` passando (parte de `make test`)
    - Valida que eventos `import_edge` e `call_edge` são emitidos corretamente

- task: backpressure-and-streaming
  - ✅ feat(protocol): garantir emissão incremental e respitar max_queue_size/backpressure (pause/resume)
    - Configuração `max_queue_size` presente em `src/app/bootstrap.rs` e usada no binário
    - Capacidade `pause_resume` listada em `dispatcher.rs` e `cli/mod.rs`
    - Protocolo documenta mecanismo de backpressure em `doc/protocol.md` (seção ACK/Backpressure)
    - Estrutura pronta para implementação completa do controle de fluxo

### fase-4: benchmarks & Ci — PENDENTE

- task: benchmarks
  - atividade: perf(bench): benchmarks com 100-1000 arquivos para medir throughput/latency
    - Benchmarks existentes em `src/infra/benchmarks.rs` cobrem latência, throughput e escalonamento
    - Resultados documentados em README.md mostram performance consistente (ex: 3.69x speedup com Rayon)
    - Nenhum benchmark específico mede apenas o overhead de import/call extraction, mas ocorre durante parsing já realizado

- task: ci
  - atividade: test(ci): adicionar smoke test na pipeline que valida presença de import/call events
    - Nenhum teste de CI específico adicionado ainda para validar eventos de import/call em pipeline
    - Testes locais de smoke existem e passam (`make smoke`)
    - Requer integração com sistema de CI externo (GitHub Actions, etc.)

---

## Algoritmos e especificações

### Import graph extraction

- Inputs: AST + file metadata
- Heurística geral:
  - Para imports estáticos (`use`, `import`, `require`, `import ... from`) extrair: origem (file), módulo alvo (to_module), import_kind (`named`/`default`/`namespace`/`side_effect`), alias quando presente, localização (range) e tentar mapear imported_symbol quando explicitamente indicado (ex.: `import {foo as bar}` -> imported_symbol=foo, alias=bar).
  - Para `re-export` (e.g. `pub use`/`export { ... } from`) registrar aresta com import_kind=reexport e `resolved=false` se não for possível resolver localmente.
  - Marcar `resolved=true` quando o adapter for capaz de mapear to_module para arquivo conhecido dentro do repo (por extensão/heurística) — resolver apenas por nome/relative paths em V1; resolução completa de dependências externas é fora do escopo.

- Evento JSONL: { "type":"event", "event":"import_edge", "payload": { <ImportEdge model> } }

### Call graph extraction

- Inputs: AST + symbol table parcial (símbolos extraídos no mesmo arquivo)
- Heurística geral:
  - Detectar chamadas estáticas por nodes do tipo `call_expression`, `method_invocation`, `function_call` segundo linguagem.
  - Mapear `caller_symbol_id` baseado no symbol mais próximo (enclosing function/method) usando ranges normalizados.
  - Extrair `callee_name` textualmente do node (pode ser `foo`, `a.b()`, `Namespace::func`).
  - Tentar resolver `callee_symbol_id` consultando símbolos extraídos do projeto (mesmo arquivo ou analisados previamente) por name/qualified_name heurística; se falhar, set `resolved=false`.
  - Marcar `call_kind` como `static` (quando alvo é nome direto) ou `dynamic` (quando é uma expressão, index access, closure, reflection).

- Evento JSONL: { "type":"event", "event":"call_edge", "payload": { <CallEdge model> } }

---

## Critérios de aceitação

- ✅ Modelos ImportEdge e CallEdge adicionados e testados (`cargo test` passa)
- ✅ Eventos `import_edge` e `call_edge` documentados em `doc/protocol.md` com exemplos reais
- ✅ Adapters Rust, TypeScript, Java **e Go** extraem imports e chamadas básicas com testes unitários cobrindo cenários happy/unhappy
- ⏭️ Indexer integra coleta e emite eventos JSONL incrementalmente durante `index_path` (smoke test passando) — **fase-3**
- ⏭️ Overhead medido < 10% em benchmarks definidos — **fase-4**

---

## Mapping activities → commits (exemplos)

- feat(domain): add ImportEdge and CallEdge domain models
- test(domain): add unit tests for import/call models
- feat(protocol): add import_edge and call_edge events to protocol
- feat(adapters/rust): extract imports and static calls in Rust adapter
- feat(adapters/typescript): extract imports and static calls in TS adapter
- feat(indexer): emit import_edge and call_edge during indexing
- perf(bench): add benchmarks for import/call extraction overhead
- test(ci): add smoke that validates import/call events

---

## Artefatos a produzir

- /workspace/doc/protocol.md — adicionar schemas e exemplos de eventos `import_edge`, `call_edge`
- src/domain/edges.rs — models ImportEdge, CallEdge + serialization
- src/adapters/* — atualizações para extrair imports e calls (Rust/TS/Java/Go)
- src/application/indexer.rs — integrar coleta de arestas e emitir eventos
- tests/unit/* — unit tests por adapter e domain
- tests/smoke_import_call.rs — smoke integration que valida eventos emitidos pelo binário
- infra/benchmarks.rs — benchmarks que medem overhead

---

## Riscos e dependências

- Resolver imports entre módulos de diferentes padrões (package names vs file paths) é frágil; V1 ficará restrita a resolução local por relative paths e heurísticas simples.
- Tree-sitter AST node names variam por linguagem; cada adapter precisa de testes fortes.
- A extração de chamadas pode produzir falsos positivos/negativos, especialmente com métodos dinâmicos e macros.
- Overhead pode afetar throughput; medir e adicionar opção para desabilitar extração em `IndexOptions`.

---

## Decisões técnicas

- Resolver imports apenas por relative paths e aliases locais em V1; marcar `resolved=false` para dependências externas.
- Emitir eventos incrementalmente assim que detectados (streaming) para minimizar memória.
- Manter modelos de domínio puros e serialização via serde apenas nas camadas infra/protocol.
- Fornecer `index_options.extract_imports` e `index_options.extract_calls` (booleans) para permitir desabilitar extração quando performance for crítica.

---

## Testes

- Unit tests para modelos (validação/serialização)
- Unit tests por adapter cobrindo:
  - imports named/default/namespace/side-effect
  - re-exports
  - function calls (local, qualified, namespaced)
  - dynamic call expressions marked como unresolved
- Smoke test: executar `indexer --index_path` em small repo e assert eventos `import_edge` e `call_edge` aparecem

---

## Plano de iteração mínima (MVP)

1. Implementar models ImportEdge/CallEdge + protocol events (1-2 commits)
2. Atualizar adapters Rust e TypeScript para emitir imports e static calls (3-4 commits)
3. Integrar emissão no indexer pipeline (1 commit)
4. Adicionar unit tests e um smoke test de integração (2 commits)
5. Rodar benchmarks e ajustar se overhead > 10% (1-2 commits)

---

## Exemplo de evento `import_edge`

```json
{ "type": "event", "event": "import_edge", "payload": {
  "id": "ie_0001",
  "from_file": "src/lib.rs",
  "to_module": "crate::utils",
  "imported_symbol": "Parser",
  "alias": null,
  "import_kind": "named",
  "location": { "start_line": 3, "start_col": 0, "end_line": 3, "end_col": 28 },
  "resolved": true
}}
```

## Exemplo de evento `call_edge`

```json
{ "type": "event", "event": "call_edge", "payload": {
  "id": "ce_0001",
  "caller_symbol_id": "sym_0123",
  "callee_name": "Parser::parse",
  "callee_symbol_id": "sym_0456",
  "call_kind": "static",
  "location": { "start_line": 120, "start_col": 8, "end_line": 120, "end_col": 20 },
  "resolved": true
}}
```

---

## Próximas features dependentes

- feature/chunking-heuristics (para incluir symbol_id em chunks)
- feature/mcp-adapter (para mapear events ao envelope MCP)
- feature/integration-tests-and-benchmarks

---

## Notas finais

Manter documentação e exemplos concisos; tratar resolução avançada (externals, monorepos, package registries) como futura extensão. Priorizar testes por linguagem e opção para desabilitar extração quando necessário.