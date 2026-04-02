# Próxima feature: feature/tree-sitter-adapters (planejamento inicial)

### Objetivo
Implementar adaptação de Tree-sitter por linguagem: parsers, extração de símbolos, e APIs que forneçam ParsedFile e Symbol extraction para o indexer.

---

## Fases e tarefas

### fase-1: adapters scaffold — ✅ CONCLUÍDA

| Status | Task | Atividade | Commits |
|---|---|---|---|
| ✅ | adapters/api | trait `LanguageAdapter` definida com `parse_source` + `extract_symbols` + `box_clone` | `2151c55` |
| ✅ | adapters/api | testes unitários com mock adapter (ParserPool tests) | `258ea57` |
| ✅ | adapters/rust | adapter Rust com `tree-sitter-rust` (functions, structs, enums, traits, impls, mods, uses, consts, statics) | `b66c5de`, `5d40661` |
| ✅ | adapters/rust | 16 testes unitários (nested scopes, signatures, line ranges) | `5d40661`, `d9c66b3` |
| ✅ | adapters/typescript | adapter TypeScript/JS com `tree-sitter-javascript` (functions, classes, methods, imports, exports, variables) | `d9c66b3`, `26c468a` |
| ✅ | adapters/typescript | 14 testes unitários | `d9c66b3`, `26c468a` |
| ✅ | adapters/java (adicional) | adapter Java com `tree-sitter-java` (methods, classes, enums, interfaces, constructors, fields, imports) | `3e4ca11` |
| ✅ | adapters/java | 14 testes unitários | `3e4ca11` |
| ✅ | parser-pool | refatorar ParserPool: `DashMap<String, Arc<dyn LanguageAdapter>>` | `4fe5671` |
| ✅ | parser-pool | 3 testes unitários básicos + 9 testes de integração multi-linguagem | `4fe5671`, `258ea57` |
| ✅ | integration | linguagem detection por extensão em `indexer.rs` (`detect_language()`) | `4fe5671` |
| ✅ | integration | bootstrap wiring: registro de todos adapters no Registry e ParserPool | `4fe5671` |
| ❌ | adapters/python | não implementado (opcional, descartado) | — |

### fase-2: integration & performance — EM ANDAMENTO

| Status | Task | Atividade | Observação |
|---|---|---|---|
| ✅ | parser-pool-integration | integrar adapters ao ParserPool | Done via `4fe5671` |
| ✅ | parser-pool-integration | medir latência e throughput | Done (20 benchmarks) |
| ✅ | symbol-normalization | mapear símbolos extraídos para o modelo Symbol | Done (`domain/normalize.rs`) |
| ✅ | symbol-normalization | testar nested symbols e overloaded | Done (23 unit tests) |
| ✅ | smoke-test | smoke test multi-linguagem | Done (`tests/smoke_multi_lang.rs`) |
| ✅ | benchmarks | benchmark com 100-500 arquivos | Done (4 novos testes de escala) |
| ✅ | docs | atualizar indexer_spec.md com LanguageAdapter API | Done |

---

### Critérios de aceitação

| Critério | Status |
|---|---|
| Trait `LanguageAdapter` definido e testado | ✅ |
| Implementação funcional para Rust + testes (16) | ✅ |
| Implementação funcional para TypeScript/JS + testes (14) | ✅ |
| Implementação funcional para Java (adicional) + testes (14) | ✅ |
| ParserPool usando DashMap, thread-safe | ✅ |
| ParserPool integração tests (9) | ✅ |
| Testes totais: 126 passando, 0 falhas | ✅ |
| Compilação com feature flag (`--features parsing`) | ✅ |
| Symbol normalization module | ✅ Done (`domain/normalize.rs`, 23 tests) |
| Smoke test multi-linguagem (small repo) | ✅ Done (`tests/smoke_multi_lang.rs`, 11 tests) |
| Perf measurement com 100-1k files | ✅ Done (`infra/benchmarks.rs`, 20 tests) |

---

### Riscos e dependências

- tree-sitter grammars crates (`tree-sitter-rust`, `tree-sitter-javascript`, `tree-sitter-python`, `tree-sitter-java`) aumentam o tempo de build; avaliar feature flags e optional-deps.
- Variações entre ASTs e nomes de nós por linguagem exigirão adaptadores específicos e testes extensivos.
- Cross-compilation para Windows/macOS pode exigir build tooling adjustments for tree-sitter C dependencies.
- `tree-sitter-java` pode ter menor maturidade que linguagens populares (rust/TS) — verificar estabilidade do crate antes de integrar.

#### Decisões técnicas

- **Config::from_file** usa `serde_json::Value` ao invés de `#[derive(Deserialize)]` em `Config`.
  - Motivo: evitar acoplar o struct de configuração ao ecossistema serde via derive, mantendo-o como um domain struct puro.
  - Extração manual via `serde_json::Value.get()` com erros claros (`MissingField`, `InvalidValue`).
  - Alternativa rejeitada: `#[derive(serde::Deserialize)]` — válido para projetos com serde-first, mas preferimos separar parsing de JSON do modelo.

### Plano de trabalho (iteração mínima)

1. ~~Definir o trait LanguageAdapter~~ ✅
2. ~~Implementar adapter-rust com tree-sitter-rust para parsing básico~~ ✅
3. ~~Implementar `extract_symbols` para Rust~~ ✅
4. ~~Adicionar TypeScript adapter com tree-sitter-javascript~~ ✅
5. ~~Adicionar Java adapter com tree-sitter-java~~ ✅
6. ~~Integrar adapters ao ParserPool~~ ✅
7. **Smoke test multi-linguagem** e update da spec — ✅ Done (`tests/smoke_multi_lang.rs`, `doc/indexer_spec.md`)

### Artefatos a produzir

- src/adapters/mod.rs (trait + registration) ✅
- src/adapters/rust.rs (implementation) ✅ full
- src/adapters/typescript.rs (implementation) ✅
- src/adapters/java.rs (implementation) ✅
- tests unitários por adapter ✅ (16 rust + 14 ts + 14 java = 44 tests)
- doc/indexer_spec.md additions: Tree-sitter adapters API and examples ✅ Done
- smoke integration que roda indexer em um pequeno repo multi-linguagem ✅ Done (`tests/smoke_multi_lang.rs`)
- ✅ tree-sitter grammars crates compilam com feature flags e optional-deps
- ✅ Variações entre ASTs mitigadas por testes unitários extensivos por linguagem
- ⚠️ Cross-compilation para Windows/macOS pode exigir build tooling adjustments (não testado)

---

### Artefatos produzidos

**Código:**
| Arquivo | Descrição |
|---|---|
| `src/adapters/mod.rs` | Trait `LanguageAdapter` + registration scaffolding |
| `src/adapters/rust.rs` | Rust adapter (tree-sitter-rust) |
| `src/adapters/rust_tests.rs` | 16 testes unitários Rust |
| `src/adapters/typescript.rs` | TypeScript/JS adapter (tree-sitter-javascript) |
| `src/adapters/typescript_tests.rs` | 14 testes unitários TypeScript/JS |
| `src/adapters/java.rs` | Java adapter (tree-sitter-java) |
| `src/adapters/java_tests.rs` | 14 testes unitários Java |
| `src/infra/parser_pool.rs` | ParserPool com DashMap + 3 unit + 9 integration tests |
| `src/application/indexer.rs` | `detect_language()` por extensão |
| `src/app/bootstrap.rs` | Registro de todos adapters
| `src/domain/normalize.rs` | Symbol normalization (qualified names, overload detection) + 23 tests
| `src/infra/benchmarks.rs` | ParserPool benchmarks + 20 tests
| `tests/smoke_multi_lang.rs` | Multi-language smoke integration tests (11 tests) |

**Configuração:**
| Arquivo | Descrição |
|---|---|
| `Cargo.toml` | Features `parsing` com `tree-sitter-rust`, `tree-sitter-javascript`, `tree-sitter-java` |

**Decisões técnicas documentadas:**
1. Não usar `extern "C"` para tree-sitter grammars — usar `crate::language()` (evita linker issues)
2. Pular ERROR nodes do parser JavaScript (sintaxe TS não reconhecida)
3. Nomes de fields Java/JS extraídos de `variable_declarator` children
4. `DashMap` para pool thread-safe lock-free
5. Detecção de linguagem por extensão de arquivo

---

### Testes

Para rodar todos os testes:

```bash
cd rust_indexer && cargo test --features parsing
```

Total: **126 testes passando** (fase-1 adapters = 44 adapter tests + 12 pool tests + 23 normalize tests + 20 benchmark tests + 27 outros tests)
