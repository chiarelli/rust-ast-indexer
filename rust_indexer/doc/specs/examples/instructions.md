# Feature: examples - Usage Examples and Integration Samples

## Objetivo

Criar exemplos de uso práticos do rust_indexer para demonstrar integração com diferentes consumidores (Node.js, Python, Go) e casos de uso comuns (indexação completa, incremental, streaming).

---

## Descrição do Usuário

"O projeto está completo (92% no planning_state.json). Criar uma feature de examples com exemplos de uso do rust_indexer para demonstrar integração com diferentes consumidores (Node.js, Python, etc)."

## Contexto

O rust_indexer é um indexador de código-fonte que opera via stdio com protocolo JSONL. Para que novos usuários possam integrar facilmente, precisamos exemplos práticos que mostrem:
- Como enviar comandos via stdio
- Como processar eventos recebidos
- Como integrar com diferentes linguagens (Node.js, Python, Go)
- Casos de uso comuns (indexação completa, incremental, filtros)

## Observações

- O protocolo JSONL já está estável (base version 1.0.0)
- Modo MCP já implementado via `--mcp` flag
- Suporte a comandos: list_languages, index_path, stop, status, incremental_index, dry_run, list_files
- Eventos emitidos: job_started, job_completed, chunk_emitted, import_edge, call_edge, pause, resume, error
- O binário está em `rust_indexer/target/debug/rust_indexer` (após `cargo build`)

---

## Fases e tarefas

### fase-1: Node.js Examples — ⏭️ PENDENTE

| Status | Task | Atividade |
|--------|------|-----------|
| ⬜ | nodejs/basic | Criar `examples/nodejs/basic_indexer.js` - exemplo mínimo |
| ⬜ | nodejs/incremental | Criar `examples/nodejs/incremental_indexer.js` - indexação com Git |
| ⬜ | nodejs/tests | Criar testes para exemplos Node.js |

### fase-2: Python Examples — ⏭️ PENDENTE

| Status | Task | Atividade |
|--------|------|-----------|
| ⬜ | python/basic | Criar `examples/python/basic_indexer.py` - exemplo mínimo |
| ⬜ | python/streaming | Criar `examples/python/streaming_processor.py` - processador com backpressure |
| ⬜ | python/tests | Criar testes para exemplos Python |

### fase-3: Go Example — ⏭️ PENDENTE

| Status | Task | Atividade |
|--------|------|-----------|
| ⬜ | go/indexer | Criar `examples/go/indexer.go` - exemplo em Go |

### fase-4: Integration Scripts — ⏭️ PENDENTE

| Status | Task | Atividade |
|--------|------|-----------|
| ⬜ | ci/script | Criar `examples/scripts/build_and_test.sh` - script CI |
| ⬜ | docker/compose | Criar `examples/docker/docker-compose.yml` - containerização |

---

## Algoritmos e especificações

### Protocolo JSONL via stdio

- Inputs: comando JSON via stdin, path do repositório como argumento
- Heurística:
  1. Iniciar processo child do rust_indexer
  2. Enviar comandos JSON via stdin (formato JSONL)
  3. Ler eventos do stdout (streaming JSONL)
  4. Processar eventos conforme tipo (chunk_emitted, job_completed, etc.)
- Output: Eventos estruturados em JSON

### Streaming com Backpressure

- Inputs: Stream de eventos do stdout
- Heurística:
  1. Thread separada para leitura de stdout
  2. Buffer limitado para controle de backpressure
  3. Enviar pause/resume conforme capacidade do buffer
- Output: Eventos processados com controle de fluxo

---

## Mapping activities → commits (exemplos)

- feat(examples/nodejs): adiciona exemplo básico Node.js
- test(examples/nodejs): adiciona testes para exemplos Node.js
- feat(examples/python): adiciona exemplo Python
- test(examples/python): adiciona testes para exemplos Python
- feat(examples/go): adiciona exemplo Go
- feat(examples/docker): adiciona Docker compose

## Critérios de aceitação

- ⏭️ Cada exemplo executa sem erros e demonstra funcionalidade
- ⏭️ Exemplos cobrem: list_languages, index_path, e processamento de eventos
- ⏭️ Node.js examples funcionam com Node.js 18+
- ⏭️ Python examples funcionam com Python 3.8+
- ⏭️ Go example compila com Go 1.20+
- ⏭️ Docker compose inicia e funciona corretamente

---

## Artefatos a produzir

```
examples/
├── nodejs/
│   ├── basic_indexer.js        # Exemplo mínimo Node.js
│   ├── incremental_indexer.js  # Indexação incremental com Git
│   └── package.json            # Dependências Node.js
├── python/
│   ├── basic_indexer.py        # Exemplo mínimo Python
│   └── streaming_processor.py  # Processador com backpressure
├── go/
│   └── indexer.go              # Exemplo Go (stdlib only)
├── scripts/
│   └── build_and_test.sh       # Script CI
└── docker/
    └── docker-compose.yml      # Containerização
```

---

## Decisões técnicas

1. **Node.js**: Usar `child_process.spawn` com `stdio: ['pipe', 'pipe', 'pipe']`
2. **Python**: Usar `subprocess.Popen` com threading para leitura de stdout
3. **Go**: Usar `os/exec` comgoroutines para streaming
4. **Docker**: imagem base `rust:alpine` para o indexer
5. **Testes**: cada exemplo inclui script de teste que valida output

---

## Protocolo de Referência

O protocolo JSONL está documentado em `doc/protocol.md`. Exemplo de comando:

```json
{"command":"list_languages","payload":{}}
```

Resposta:
```json
{"type":"event","event":"capabilities","payload":{"languages":["rust","typescript","javascript","java","go"]}}
```

---

## Testes

Cada exemplo deve ser executável standalone:
```bash
# Node.js
cd examples/nodejs && node basic_indexer.js /path/to/repo

# Python
cd examples/python && python basic_indexer.py /path/to/repo

# Go
cd examples/go && go run indexer.go /path/to/repo
```

---

## Riscos e dependências

- **Risco 1**: Versões diferentes de Node.js/Python/Go podem ter comportamento diferente com subprocessos
- **Risco 2**: Docker compose pode não funcionar em ambientes sem daemon Docker
- **Dependência 1**: rust_indexer binário deve estar compilado antes de rodar exemplos
- **Dependência 2**: Protocolo JSONL deve manter compatibilidade com versão 1.0.0

---

## Plano de iteração mínima (MVP)

1. Fase 1 completa - Node.js examples funcionando (2-3 commits)
2. Fase 2 completa - Python examples funcionando (2 commits)
3. Fase 3 completa - Go example funcionando (1 commit)
4. Fase 4 completa - Scripts de integração (1 commit)

---

## Notas finais

Manter exemplos simples e focados. Cada exemplo deve demonstrar exatamente um caso de uso. Documentar prerequisites (Node.js, Python, Go, Docker) em comments.