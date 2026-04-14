# Feature: feature/mcp-adapter

## Objetivo

Adicionar compatibilidade MCP (Model Context Protocol) ao rust_indexer, permitindo que o indexador sejaexposto como ferramenta MCP via stdio (JSON-RPC 2.0).

O foco da V1 é compatibilidade stdio - o indexer funciona como tool MCP que responds a JSON-RPC requests via stdin e emits JSONL events via stdout.

---

## Fases e tarefas

### fase-1: MCP Schema e Tool Definitions — 👷 EM PROGRESSO

- task: mcp/schema
  - atividade: feat(app): definir MCP tool schema para rust_indexer
  - atividade: test(app): unit tests para tool schema validation

- task: mcp/mapping
  - atividade: feat(app): mapear capabilities existentes para MCP tools
  - atividade: test(app): unit tests para mapping
    - `list_languages` → MCP tool
    - `index_path` → MCP tool
    - `stop` → MCP tool
    - `status` → MCP tool

### fase-2: MCP Stdio Adapter — ⏭️ PENDENTE

- task: mcp/stdio
  - atividade: feat(infra): implementar MCP stdio adapter
  - atividade: test(infra): unit tests para JSON-RPC parsing
  - atividade: test(smoke): smoke test que valida resposta MCP

### fase-3: Integração — ⏭️ PENDENTE

- task: mcp/integration
  - atividade: feat(cli): adicionar modo MCP ao CLI
  - atividade: test(integration): integration test com MCP client

---

## Algoritmos e especificações

### MCP Tool Schema

- Ferramentas definidas:
  - `list_languages`: Retorna lista de linguagens suportadas
  - `index_path`: Executa indexação de diretório
  - `stop`: Interrompeindexação em andamento
  - `status`: Retorna status atual do indexer

- Input Schema (JSON Schema formato):
```json
{
  "list_languages": { "type": "object", "properties": {} },
  "index_path": {
    "type": "object",
    "properties": {
      "path": { "type": "string" },
      "options": { "type": "object" }
    }
  },
  "stop": { "type": "object", "properties": {} },
  "status": { "type": "object", "properties": {} }
}
```

### JSON-RPC 2.0 Request/Response

- Request (stdin):
```json
{ "jsonrpc": "2.0", "id": 1, "method": "list_languages", "params": {} }
```

- Response (stdout):
```json
{ "jsonrpc": "2.0", "id": 1, "result": { "languages": ["rust", "typescript", ...] } }
```

- Error Response:
```json
{ "jsonrpc": "2.0", "id": 1, "error": { "code": -32600, "message": "Invalid Request" } }
```

### Eventos JSONL durante index_path

- Durante `index_path`, emitir eventos JSONL como notifications (sem id):
```json
{ "jsonrpc": "2.0", "method": "indexing/file", "params": { "file": "src/main.rs" } }
{ "jsonrpc": "2.0", "method": "indexing/chunk", "params": { "chunk_id": "...", ... } }
{ "jsonrpc": "2.0", "method": "indexing/complete", "params": { "files": 42, "chunks": 156 } }
```

---

## Critérios de aceitação

- ⏭️ Tool schema definido e testado (`cargo test` passa)
- ⏭️ 4 capabilities mapeadas para MCP tools com input schemas
- ⏭️ Adapter stdio responde a JSON-RPC requests com respostas válidas
- ⏭️ Eventos emitidos durante indexação como JSON-RPC notifications
- ⏭️ Smoke test passando que valida integração MCP

---

## Mapping activities → commits (exemplos)

- feat(app): define MCP tool schema
- feat(app): map capabilities to MCP tools
- feat(infra): implement MCP stdio adapter
- feat(cli): add MCP mode to CLI
- test(app): unit tests for tool schema
- test(infra): unit tests for JSON-RPC
- test(smoke): smoke test for MCP integration

---

## Artefatos a produzir

- src/app/mcp_tool.rs — definições de tool schema e mapping
- src/infra/mcp_adapter.rs — implementação do adapter stdio
- src/cli/mod.rs — opção --mcp para ativar modo
- tests/unit/mcp_*.rs — unit tests
- tests/smoke_mcp.rs — smoke test de integração

---

## Riscos e dependências

- JSON-RPC 2.0 parsing deve ser robusto (malformed requests)
- Modo MCPmutuamente exclusivo com modo JSONL atual? Decisão: modo dual (detecta request type)
- Streaming de eventos durante indexação deve respeitar backpressure

---

## Decisões técnicas

- Modo MCP ativado via flag `--mcp` no CLI
- Request detection: se primeiro char é `{` e contém "jsonrpc", treat as MCP; else JSONL
- Manter compatibilidade com JSONL legacy (default mode)
- Eventos são emitidos como JSON-RPC notifications (sem id em response)

---

## Testes

- Unit tests para tool schema validation
- Unit tests para JSON-RPC request/response parsing
- Unit tests para capability mapping
- Smoke test: executar indexer em modo MCP e validar responses

---

## Plano de iteração mínima (MVP)

1. Definir MCP tool schema + unit tests (1-2 commits)
2. Mapear 4 capabilities para tools (1 commit)
3. Implementar MCP stdio adapter + unit tests (2 commits)
4. Adicionar modo CLI --mcp + smoke test (1-2 commits)

---

## Exemplo de sessão MCP

### Request: list_languages
```json
{ "jsonrpc": "2.0", "id": 1, "method": "list_languages", "params": {} }
```

### Response
```json
{ "jsonrpc": "2.0", "id": 1, "result": { "languages": ["rust", "go", "python", "typescript", "java"] } }
```

### Request: index_path
```json
{ "jsonrpc": "2.0", "id": 2, "method": "index_path", "params": { "path": "./src" } }
```

### Notifications (streamed)
```json
{ "jsonrpc": "2.0", "method": "indexing/file", "params": { "file": "src/main.rs", "language": "rust" } }
{ "jsonrpc": "2.0", "method": "indexing/symbol", "params": { "symbol_id": "sym_001", "kind": "function", "name": "main" } }
{ "jsonrpc": "2.0", "method": "indexing/chunk", "params": { "chunk_id": "chk_001", "size": 1234 } }
{ "jsonrpc": "2.0", "method": "indexing/complete", "params": { "files": 15, "chunks": 42 } }
```

### Final Response
```json
{ "jsonrpc": "2.0", "id": 2, "result": { "files": 15, "chunks": 42 } }
```

---

## Notas finais

Manter retrocompatibilidade com JSONL atual. Modo MCP é alternativo, não substitui modo JSONL legacy por padrão.