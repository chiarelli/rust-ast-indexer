# Feature: Tree-sitter adapters

### Objetivo
Implementar adaptação de Tree-sitter por linguagem: parsers, extração de símbolos, e APIs que forneçam `ParsedFile` e `Symbol` extraction para o indexer.

---

## Status

| Adapter | Status | Arquivo |
|---|---|---|
| Trait `LanguageAdapter` | ✅ Definido | `adapters/mod.rs` |
| Rust | ✅ Scaffold parsing | `adapters/rust.rs` |
| TypeScript/JS | ❌ Pendente | |
| Python | ❌ Opcional | |
| Java | ❌ Pendente | |
| Symbol extraction real (todas) | ❌ Pendente | Placeholder retorna `vec![]` |
| Integração ParserPool | ❌ Pendente | |
| Smoke multi-linguagem | ❌ Pendente | |

---

## Fases e tarefas

### fase-1: adapters scaffold

- **task: adapters/api** (✅ completo)
  - ~~feat(adapters): definir trait LanguageAdapter~~ — Trait com `parse_source`, `extract_symbols`, `box_clone`
  - ~~test(adapters): testes unitários com mock/stub~~

- **task: adapters/rust** (⚡ parsing scaffold, extraction pendente)
  - ~~feat(adapter-rust): adapter mínimo para Rust usando tree-sitter-rust~~ — Parsing implementado, `extract_symbols` é placeholder
  - ~~test(adapter-rust): testes unitários com snippets~~
  - **TODO: Implementar `extract_symbols`** — caminhar AST retornando funções, structs, impls, enums

- **task: adapters/typescript** (❌ pendente)
  - feat(adapter-ts): implementar adapter mínimo para TypeScript/JS usando `tree-sitter-javascript`
  - test(adapter-ts): testes unitários com exports/imports, functions, classes

- **task: adapters/python** (❌ opcional)
  - feat(adapter-py): adapter Python mínimo (`tree-sitter-python`)
  - test(adapter-py): testes unitários para top-level functions e classes

- **task: adapters/java** (❌ pendente — nova adição)
  - feat(adapter-java): adapter Java mínimo (`tree-sitter-java`)
  - test(adapter-java): testes unitários para classes, métodos, interfaces, enums

### fase-2: integration & performance

- **task: parser-pool-integration**
  - feat(pool): integrar adapters ao ParserPool existente e garantir isolamento por thread
  - perf(pool): medir latência e throughput em repositórios pequenos (100-1k files)

- **task: symbol-normalization**
  - feat(norm): mapear símbolos extraídos para o modelo Symbol (id, kind, name, qualified_name, file_path, range, signature)
  - test(norm): testar casos de nested symbols e overloaded names

---

### Critérios de aceitação

- Trait LanguageAdapter definido e documentado em doc/indexer_spec.md (adicionar seção "Tree-sitter adapters API").
- Implementações funcionais para Rust, TypeScript e Java com cobertura unitária.
- ParserPool usa adapters para parsing em múltiplas threads sem data races.
- Símbolos extraídos mapeados para o modelo Symbol usado no indexer e usados para gerar chunks.

### Riscos e dependências

- tree-sitter grammars crates (`tree-sitter-rust`, `tree-sitter-javascript`, `tree-sitter-python`, `tree-sitter-java`) aumentam o tempo de build; avaliar feature flags e optional-deps.
- Variações entre ASTs e nomes de nós por linguagem exigirão adaptadores específicos e testes extensivos.
- Cross-compilation para Windows/macOS pode exigir build tooling adjustments for tree-sitter C dependencies.
- `tree-sitter-java` pode ter menor maturidade que linguagens populares (rust/TS) — verificar estabilidade do crate antes de integrar.

### Plano de trabalho (iteração mínima)

1. ~~Definir o trait LanguageAdapter~~ ✅
2. ~~Implementar adapter-rust com tree-sitter-rust para parsing básico~~ ✅
3. **Implementar `extract_symbols` para Rust** — caminhar AST (próximo passo)
4. **Adicionar TypeScript adapter** com tree-sitter-javascript
5. **Adicionar Java adapter** com tree-sitter-java
6. **Integrar adapters ao ParserPool** em vez de criar parsers por chamada
7. **Smoke test multi-linguagem** e update da spec

### Artefatos a produzir

- src/adapters/mod.rs (trait + registration) ✅
- src/adapters/rust.rs (implementation) ✅ scaffold
- src/adapters/typescript.rs (implementation)
- src/adapters/java.rs (implementation — **nova adição**)
- tests unitários por adapter
- doc/indexer_spec.md additions: Tree-sitter adapters API and examples
- smoke integration que roda indexer em um pequeno repo multi-linguagem