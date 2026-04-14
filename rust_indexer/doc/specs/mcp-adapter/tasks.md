# MCPTooling - Tasks Detalhadas

## Task 1: Define MCP Tool Schema
- Fase: fase-1
- Arquivos: src/app/mcp_tool.rs (novo)
- Descrição: Definir structs para MCP tool schema com name, description, inputSchema
- Critério de sucesso: Tool schema compila e passa em unit tests
- Dependências: Nenhuma
- Heurísticas:
  - Usar JSON Schema draft-07 para input schemas
  - 4 tools: list_languages, index_path, stop, status

## Task 2: Map Capabilities to MCP Tools
- Fase: fase-1
- Arquivos: src/app/mcp_tool.rs
- Descrição: Mapear capabilities existentes para MCP tools
- Critério de sucesso: 4 tools mapeadas com testes passando
- Dependências: Task 1

## Task 3: Implement MCP Stdio Adapter
- Fase: fase-2
- Arquivos: src/infra/mcp_adapter.rs (novo)
- Descrição: Implementar parsing JSON-RPC 2.0 e dispatcher
- Critério de sucesso: Responde a requests válido com resultado válido
- Dependências: Task 2
- Heurísticas:
  - Parse JSON-RPC 2.0 requests
  - Dispatch para tool handler apropriado
  - Emitir responses e notifications

## Task 4: Add MCP Mode to CLI
- Fase: fase-3
- Arquivos: src/cli/mod.rs, src/bin.rs
- Descrição: Adicionar flag --mcp para ativar modo MCP
- Critério de sucesso: --mcp flag funciona corretamente
- Dependências: Task 3

## Task 5: Integration Tests
- Fase: fase-3
- Arquivos: tests/smoke_mcp.rs (novo)
- Descrição: Smoke test que executa indexer em modo MCP
- Critério de sucesso: Teste passa com respostas válidas
- Dependências: Task 4