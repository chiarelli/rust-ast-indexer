# Próxima feature: feature/chunking-heuristics (planejamento inicial)

### Objetivo

Implementar estratégias de geração de chunks a partir de símbolos e código-fonte, definindo como dividir arquivos indexados em unidades menores e semanticamente coerentes para consumo por LLMs ou outras ferramentas. Os chunks devem respeitar limites de tamanho, preservar contexto (imports, assinaturas, escopos) e permitir reconstrução incremental.

---

## Fases e tarefas

### fase-1: heurísticas básicas e modelo de chunk — CONCLUÍDA

| Status | Task | Atividade | Commits |
|---|---|---|---|
| ✅ | domain/chunk-model | definir struct `Chunk` com campos obrigatórios (id, file_path, symbol_ids, content, start_line, end_line, metadata) | 121f6ce + atual |
| ✅ | domain/chunk-model | testes unitários para Chunk (creation, validation, display) | atual |
| ✅ | chunking/symbol-boundary | chunker que alinha chunks a limites de símbolos (functions, classes, methods) | atual |
| ✅ | chunking/symbol-boundary | testes: chunk único por símbolo, símbolos aninhados, símbolos sem corpo | atual |
| ✅ | chunking/size-limits | limitar chunks por número de linhas (configurável, ex.: max_lines=200) | atual |
| ✅ | chunking/size-limits | split inteligente: preferir quebrar entre símbolos ao ultrapassar limite | atual |
| ✅ | chunking/context | injetar imports e escopo pai (scope context) como prefixo do chunk | atual |
| ✅ | chunking/context | testes: imports presentes, scope chain preservado, sem duplicação excessiva | atual |
| ✅ | integration | adaptar pipeline paralelo (indexer) para usar chunker ao gerar `chunk_emitted` | atual |
| ✅ | integration | smoke test: validar que chunks emitidos têm estrutura e contexto corretos | atual |
| ✅ | docs | documentar modelo de chunk em doc/indexer_spec.md (ou doc/chunking.md) | atual |

### fase-2: estratégias avançadas e otimização — PENDENTE

| Status | Task | Atividade | Observação |
|---|---|---|---|
| ❌ | chunking/semantic | chunker semântico: agrupar símbolos relacionados (impls ↔ struct traits ↔ type) | Heurística por linguagem |
| ❌ | chunking/tokens | limitar por token count (estimativa), não apenas linhas | Usar contagem aproximada (chars/4) |
| ❌ | chunking/overlap | permitir overlap entre chunks vizinhos para preservar contexto de fronteira | Configurável (overlap_lines) |
| ❌ | chunking/fallback | fallback para arquivos sem símbolos conhecidos (ex.: config, markdown, txt) | Chunk por blocos de tamanho fixo |
| ❌ | benchmarks | medir latência e throughput do chunker com 100–500 arquivos | Adicionar a infra/benchmarks.rs |
| ❌ | smoke-test | smoke test multi-estratégia: validar diferentes chunkers em mesmo arquivo | — |

### fase-3: integração final e CI — PENDENTE

| Status | Task | Atividade | Observação |
|---|---|---|---|
| ❌ | config | expor ChunkingOptions no CLI payload (strategy, max_lines, overlap, token_limit) | — |
| ❌ | integration | wire completo: CLI → IndexOptions → Chunker → chunk_emitted | — |
| ❌ | tests/ci | testes unitários cobrindo todas estratégias + unhappy paths | >= 90% cobertura |
| ❌ | docs/atualização | atualizar indexer_spec.md com schema completo de chunk_emitted | — |

---

### Critérios de aceitação

| Critério | Status |
|---|---|
| Struct `Chunk` definida, validada e testada | ✅ |
| Chunker por limites de símbolo implementado | ✅ |
| Limite de linhas configurável com split inteligente | ✅ |
| Contexto (imports, scope chain) injetado como prefixo | ✅ |
| Pipeline integrado: chunks emitidos via `chunk_emitted` | ✅ |
| Fallback para arquivos sem símbolos | ✅ |
| Smoke test validando estrutura de chunks | ✅ |
| Documentação atualizada (indexer_spec.md ou doc/chunking.md) | ✅ |
| Testes totais: objetivo 60+ unitários + integração | ✅ |
| Compilação limpa com `--features parsing` | ✅ |
| `cargo clippy -- -D warnings` sem warnings | ❌ |

---

### Riscos e dependências

- **Dependência de adapters**: chunker depende de símbolos extraídos corretamente por `LanguageAdapter` (feature/tree-sitter-adapters consolidada).
- **Variações linguísticas**: heurísticas de split podem precisar de ajustes por linguagem (ex.: macros Rust, decorators TS).
- **Performance**: chunking adiciona overhead ao pipeline; necessário bench para validar impacto (< 5% do tempo total de indexação).
- **Token counting**: estimativa por caracteres pode ser imprecisa; avaliar se justifica integrar tokenizer real ou manter aproximação.
- **Backpressure**: chunks maiores ou em maior volume podem exigir ajustes no mecanismo de backpressure (feature/backpressure-and-resume).

#### Decisões técnicas propostas

- **Modelo Chunk imutável após criação**: evitar mutações acidentais durante pipeline paralelo.
- **Strategy pattern para chunkers**: trait `ChunkStrategy` com múltiplas implementações; selecionável por config.
- **Context prefix compartilhado**: imports e escopo pai podem ser reutilizados entre chunks do mesmo arquivo; considerar cache ou referência para evitar duplicação em memória.
- **Metadata extensível**: usar `HashMap<String, serde_json::Value>` ou struct com campos opcionais para permitir evolução sem breaking changes.
- **Limites configuráveis via payload CLI**: permitir caller ajustar `max_lines`, `strategy`, `overlap` por job.

---

### Plano de trabalho (iteração mínima)

1. ~~Definir struct `Chunk` e validações básicas~~
2. ~~Implementar symbol-boundary chunker (um símbolo = um chunk)~~
3. ~~Adicionar limite de linhas com split entre símbolos~~
4. ~~Implementar context injection (imports + scope chain)~~
5. ~~Integrar ao pipeline paralelo do indexer~~
6. ~~Smoke test: validar chunks emitidos em repo multi-linguagem~~
7. ~~Adicionar fallback para arquivos sem símbolos~~
8. ~~Documentar modelo e schema em spec~~

---

### Artefatos a produzir

- `src/domain/chunk.rs` — struct Chunk, validação, display
- `src/domain/chunk_tests.rs` — testes unitários de Chunk
- `src/application/chunking/mod.rs` — trait `ChunkStrategy`
- `src/application/chunking/symbol_boundary.rs` — implementação por símbolo
- `src/application/chunking/size_limited.rs` — implementação com limite de linhas
- `src/application/chunking/with_context.rs` — decorator para injetar contexto
- `src/application/chunking/fallback.rs` — fallback para arquivos sem símbolos
- `src/application/chunking/*/tests.rs` — testes por estratégia
- `src/application/indexer.rs` — adaptação para usar chunker
- `tests/smoke_chunking.rs` — smoke test end-to-end
- `doc/chunking.md` ou adição a `doc/indexer_spec.md` — documentação de schema e exemplos
- `src/infra/benchmarks.rs` — bench de throughput do chunker

### Decisões técnicas documentadas (a preencher)

1. Token counting opt-in — evita overhead desnecessário no path padrão; caller paga só quando precisa. Bom usar token_count como estimativa (chars/4 é fallback rápido se a lib falhar).
2. Limite de linhas configurável por chunk, default 200 — funciona bem para context windows de LLM (tipicamente 2K–8K tokens); configurável permite ajuste por caso de uso.
3. Overlap desabilitado por padrão; porém deixar meta informação do próximo chunk, para evitar funções cortadas — melhor que duplicação real: sem custo de memória extra mas com informação suficiente para reconstruir contexto se necessário. O metadata pode incluir next_chunk_id, next_symbol_id, ou continuation_hint (ex.: "function UserService::delete continua no próximo chunk").
4. Usar `chk-<hash>` para melhor performance (determinístico baseado em hash de assinatura, filepath, strategy, etc.).

---

### Modelo de Chunk (schema proposto)

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
    "tokens_estimate": 180,
    "split_from_symbol": "UserService::add"
  }
}
```

---

### Integração com CLI payload

O comando `index_path` e `incremental_index` deverão suportar `chunking_options` em `options`:

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
        "token_limit": null
      }
    }
  }
}
```

Campos opcionais com defaults sensatos para não exigir alteração de callers existentes.

---

### Testes

Para rodar todos os testes (após implementação):

```bash
cd rust_indexer && cargo test --features parsing
```

Objetivo: **60+ testes unitários** (Chunk model + strategies) + **5+ smoke/integração** + **2+ benchmarks** de chunking throughput.

---

### Mapping activities → commits (exemplo de mensagens)

```
feat(chunk): define Chunk struct and validation
test(chunk): add unit tests for Chunk model
feat(chunking): add symbol-boundary chunker strategy
test(chunking): add symbol-boundary chunker tests
feat(chunking): implement size-limited chunking with symbol splitting
test(chunking): add size-limited chunker tests
feat(chunking): add context injection decorator (imports + scope)
test(chunking): verify context prefix in chunks
feat(chunking): implement fallback chunker for unknown files
test(chunking): fallback chunker unit tests
feat(indexer): integrate chunker into parallel pipeline
test(smoke): add chunking end-to-end smoke test
feat(chunk): expose ChunkingOptions in CLI payload
docs: document chunk schema and strategies in indexer_spec.md
perf(chunking): add benchmark for chunker throughput
```

### Organização (feature → fases → tasks → atividades)

- feature/chunking-heuristics
  - fase-1: heurísticas básicas e modelo de chunk
    - task: domain/chunk-model
      - atividade (commit): feat(chunk): define Chunk struct and validation
      - atividade (commit): test(chunk): add unit tests for Chunk model
    - task: chunking/symbol-boundary
      - atividade: feat(chunking): add symbol-boundary chunker strategy
      - atividade: test(chunking): add symbol-boundary chunker tests
    - task: chunking/size-limits
      - atividade: feat(chunking): implement size-limited chunking with symbol splitting
      - atividade: test(chunking): add size-limited chunker tests
    - task: chunking/context
      - atividade: feat(chunking): add context injection decorator (imports + scope)
      - atividade: test(chunking): verify context prefix in chunks
    - task: integration
      - atividade: feat(indexer): integrate chunker into parallel pipeline
      - atividade: test(smoke): add chunking end-to-end smoke test
    - task: docs
      - atividade: docs: document chunk schema and strategies in indexer_spec.md
  - fase-2: estratégias avançadas e otimização
    - task: chunking/semantic
      - atividade: feat(chunking): add semantic grouping strategy (impl↔struct)
    - task: chunking/tokens
      - atividade: feat(chunking): implement token-count based limits
    - task: chunking/overlap
      - atividade: feat(chunking): add configurable overlap between chunks
    - task: benchmarks
      - atividade: perf(chunking): add chunker throughput benchmarks
  - fase-3: integração final e CI
    - task: config
      - atividade: feat(cli): expose ChunkingOptions in index_path payload
    - task: tests/ci
      - atividade: test(ci): add unit tests for all chunking strategies
    - task: docs/atualização
      - atividade: docs: finalize chunk schema in indexer_spec.md

Cada atividade corresponde a um commit com o título no formato: type(scope): short description
Exemplo: `feat(chunking): add symbol-boundary chunker strategy`.

Branches por task: `feature/chunking-heuristics/<task-name>` (ex.: `feature/chunking-heuristics/domain-chunk-model`).

Cada task DEVE incluir testes de unidade cobrindo cenários 'happy' e 'unhappy' (erro), e deve cobrir todas as funções/métodos envolvidos, incluindo helpers privados (via #[cfg(test)] no mesmo módulo).

Critérios de aceitação por atividade:
- Todos os testes unitários passam localmente (`cargo test`).
- `cargo clippy` deve passar sem warnings (usar `-D warnings` na CI).
- Cobertura de unidade para o módulo alvo é >= 90% (meta; documentar exceções).
- Commit está nomeado no formato `type(scope): short description`.
- Ao commitar: usar explicitamente `git add <file>` (não `git add .`).
- Testes de integração/smoke para a task rodando via binário devem existir quando aplicável.
- Documentação mínima (README ou comentário no módulo) explicando a função e como testar.
