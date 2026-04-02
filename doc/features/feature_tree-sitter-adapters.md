# Próxima feature: feature/tree-sitter-adapters (planejamento inicial)

### Objetivo
Implementar adaptação de Tree-sitter por linguagem: parsers, extração de símbolos, e APIs que forneçam ParsedFile e Symbol extraction para o indexer.

### Fases e tarefas
- fase-1: adapters scaffold
  - task: adapters/api
    - atividade: feat(adapters): definir trait LanguageAdapter { fn parse_source(&self, source: &str) -> Result<ParsedFile>; fn extract_symbols(&self, parsed: &ParsedFile) -> Vec<Symbol> }
    - atividade: test(adapters): adicionar testes unitários para o contrato do trait usando mock/stub de parser
  - task: adapters/rust
    - atividade: feat(adapter-rust): implementar adapter mínimo para Rust usando tree-sitter-rust
    - atividade: test(adapter-rust): testes unitários com snippets pequenos (funções, structs, impls)
  - task: adapters/typescript
    - atividade: feat(adapter-ts): implementar adapter mínimo para TypeScript/JS usando tree-sitter-javascript
    - atividade: test(adapter-ts): testes unitários com exemplos de exports/imports, functions
  - task: adapters/python (opcional inicial)
    - atividade: feat(adapter-py): adapter Python mínimo (tree-sitter-python)
    - atividade: test(adapter-py): testes unitários para top-level functions e classes

- fase-2: integration & performance
  - task: parser-pool-integration
    - atividade: feat(pool): integrar adapters ao ParserPool existente e garantir isolamento por thread
    - atividade: perf(pool): medir latência e throughput em repositórios pequenos (100-1k files)
  - task: symbol-normalization
    - atividade: feat(norm): mapear símbolos extraídos para o modelo Symbol (id, kind, name, qualified_name, file_path, range, signature)
    - atividade: test(norm): testar casos de nested symbols e overloaded names

### Critérios de aceitação
- Trait LanguageAdapter definido e documentado em doc/indexer_spec.md (adicionar seção "Tree-sitter adapters API").
- Implementações funcionais para Rust e TypeScript com cobertura unitária.
- ParserPool usa adapters para parsing em múltiplas threads sem data races (tests de stress ou verificação com MIRI/tsan when possible).
- Símbolos extraídos mapeados para o modelo Symbol usado no indexer e usados para gerar chunks.

### Riscos e dependências
- tree-sitter grammars crates (tree-sitter-rust, tree-sitter-javascript, tree-sitter-python) aumentam o tempo de build; avaliar feature flags e optional-deps.
- Variações entre ASTs e nomes de nós por linguagem exigirão adaptadores específicos e testes extensivos.
- Cross-compilation para Windows/macOS pode exigir build tooling adjustments for tree-sitter C dependencies.

### Plano de trabalho (iteração mínima)
1. Definir o trait LanguageAdapter no crate domain/adapters.rs e documentar na spec.
2. Implementar adapter-rust com tree-sitter-rust para parsing básico e extração de functions/structs/impls.
3. Expor parse_source + extract_symbols e escrever testes unitários.
4. Integrar adapter-rust no ParserPool e run basic end-to-end on small repo tests.
5. Repeat for TypeScript adapter.
6. Update doc/indexer_spec.md with concrete data structures and examples.

### Artefatos a produzir
- src/adapters/mod.rs (trait + registration)
- src/adapters/rust.rs (implementation)
- src/adapters/typescript.rs (implementation)
- tests unitários por adapter
- doc/indexer_spec.md additions: Tree-sitter adapters API and examples
- smoke integration that runs indexer on a small multi-language sample repo