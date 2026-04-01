# Bootstrap / Application Container (Dependency Injection)

Objetivo

Definir um bootstrap central (ApplicationContext / Container) que: 
- injeta dependências (AdapterRegistry, ParserPool, Config, Metrics, Logger),
- gerencia ciclo de vida de recursos nativos (parsers Tree-sitter),
- permite testabilidade via contextos isolados por teste/run,
- evita singletons globais espalhados.

Visão geral da arquitetura

- ApplicationContext (immutable after init) {
  - registry: Arc<Registry>
  - parser_pool: Arc<ParserPool>
  - config: Config
  - metrics: Option<Arc<Metrics>>
  - logger: Option<Arc<Logger>>
}

- Registry: pequenas API `register(lang, Box<dyn LanguageAdapter>)` e `get(lang) -> Option<Arc<dyn LanguageAdapter>>`.
  - Implementação recomendada: DashMap<String, Arc<dyn LanguageAdapter>> para alta concorrência.

- ParserPool: criado/owned pelo Context; provê Arc<ParserPool> para Indexer/handlers; teardown controlado pelo Context drop.

- Startup flow
  1. main -> load config
  2. let ctx = bootstrap::init(config)
  3. ctx.register built-in adapters (Rust, TS) or registrar plugins
  4. run_cli(ctx)

- Handlers and services
  - Recebem &Arc<ApplicationContext> (ou cloned Arc) como primeira dependência.
  - Ex.: Indexer::new(ctx.clone()) ou indexer.index_path(ctx.clone(), ...)

Test strategy

- Tests create a local Context with minimal config and register test adapters (mocks). No global state.

Migration plan (incremental)

1. Criar crate::app::bootstrap com ApplicationContext and init function.
2. Implementar Registry struct (DashMap) e substituir uso de lazy_static por Registry em adapters/mod.rs (compat shim temporário).
3. Atualizar main.rs para inicializar Context e chamar run_cli(ctx).
4. Alterar run_cli signature para receber ctx (backwards compat: provide run_cli_default wrapper that calls run_cli with bootstrap())
5. Refatorar Indexer/CLI para obter dependências do Context. Fazer em passos pequenos e manter testes verdes.

Riscos / tradeoffs

- Refactor extensivo de pontos de integração; aplicar em branch isolada (feature/bootstrap-di).
- Requer atualização de muitos testes; implemente um test-bootstrap helper.
- Melhor testabilidade e controle de lifecycle; aumento inicial de complexidade.

Recomendação sobre branch

Abrir uma feature branch dedicada: `feature/bootstrap-di`.
- Permite alterações cross-cutting isoladas, PRs pequenas e reversíveis.
- Execute a migração em commits atômicos: (1) bootstrap module + tests, (2) wiring main/run_cli, (3) migrate registry, (4) migrate parser_pool ownership, (5) cleanup globals.

Próximo passo sugerido

Criar a branch `feature/bootstrap-di` e implementar `crate::app::bootstrap` com ApplicationContext e init/config loader. Depois eu inicio a implementação se confirmar.