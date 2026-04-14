# Examples - Implementation Plan

## Estrutura de Execução
P1 ; P2 ; P3 ; P4

## Fase 1: Node.js Examples
{T1.1 || T1.2}
- T1.1: basic_indexer.js → examples/nodejs/basic_indexer.js
- T1.2: incremental_indexer.js → examples/nodejs/incremental_indexer.js

## Fase 2: Python Examples
{T2.1 || T2.2}
- T2.1: basic_indexer.py → examples/python/basic_indexer.py
- T2.2: streaming_processor.py → examples/python/streaming_processor.py

## Fase 3: Go Example
- T3.1: indexer.go → examples/go/indexer.go

## Fase 4: Integration Scripts
{T4.1 || T4.2}
- T4.1: build_and_test.sh → examples/scripts/build_and_test.sh
- T4.2: docker-compose.yml → examples/docker/docker-compose.yml

---

## Validação

```bash
# Node.js
cd examples/nodejs && node basic_indexer.js <repo_path>
# Expected: lista de linguagens + chunks indexados

# Python  
cd examples/python && python basic_indexer.py <repo_path>
# Expected: output JSONL processado

# Go
cd examples/go && go run indexer.go <repo_path>
# Expected: chunks listados em stdout

# Docker
cd examples/docker && docker-compose up
# Expected: indexer container rodando
```

---

## Dependências

- rust_indexer binário compilado (cargo build)
- Node.js 18+ (para examples nodejs)
- Python 3.8+ (para examples python)
- Go 1.20+ (para example go)
- Docker e docker-compose (para exemplo container)