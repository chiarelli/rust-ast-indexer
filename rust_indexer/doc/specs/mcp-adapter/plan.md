# MCPTooling - Implementation Plan

## Estrutura de Execução
P1 ; P2

## Fase 1: MCP Stdio Compatibility
{T1.1 || T1.2}
- T1.1: Definir MCP tool schema para rust_indexer → src/app/mcp_tool.rs
- T1.2: Mapear capabilities existentes para MCP tools → src/app/mcp_tool.rs

## Fase 2: MCP Stdio Adapter
{T2.1}
- T2.1: Implementar MCP stdio adapter → src/infra/mcp_adapter.rs

## Validação
```bash
cargo test --features parsing
make integration
```