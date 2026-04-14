# Examples - Tasks

## Task 1: Node.js Basic Example
- Fase: fase-1
- Arquivos: `examples/nodejs/basic_indexer.js`, `examples/nodejs/package.json`
- Descrição: Criar exemplo mínimo que demonstra list_languages e index_path usando child_process.spawn
- Critério: Script executa list_languages e index_path, loga chunks emitidos
- Dependências: Node.js 18+, child_process (stdlib)
- Validação: `node basic_indexer.js ./src` deve mostrar linguagens + chunks

## Task 2: Node.js Incremental Example
- Fase: fase-1
- Arquivos: `examples/nodejs/incremental_indexer.js`
- Descrição: Demonstra use_git=true com git_range para diff entre commits
- Critério: Demonstra use_git=true com git_range para diff entre commits
- Dependências: child_process, processo git disponível
- Validação: Indexa apenas arquivos modificados desde última tag/commit

## Task 3: Python Basic Example
- Fase: fase-2
- Arquivos: `examples/python/basic_indexer.py`
- Descrição: Criar exemplo equivalente ao Node.js usando subprocess.Popen
- Critério: Equivalente ao exemplo Node.js (list_languages + index_path)
- Dependências: Python 3.8+, subprocess (stdlib)
- Validação: `python basic_indexer.py ./src` deve mostrar output estruturado

## Task 4: Python Streaming Processor
- Fase: fase-2
- Arquivos: `examples/python/streaming_processor.py`
- Descrição: Processa eventos em stream com tratamento pause/resume e backpressure
- Critério: Processa eventos em stream com tratamento pause/resume
- Dependências: threading para leitura não-bloqueante
- Validação: Demonstra backpressure handling

## Task 5: Go Example
- Fase: fase-3
- Arquivos: `examples/go/indexer.go`
- Descrição: Demonstra criação de processo child e leitura de stdout usando stdlib
- Critério: Demonstra criação de processo child e leitura de stdout
- Dependências: Go 1.20+, stdlib only (os/exec, bufio)
- Validação: `go run indexer.go ./src` compila e executa

## Task 6: CI Integration Script
- Fase: fase-4
- Arquivos: `examples/scripts/build_and_test.sh`
- Descrição: Script que roda indexer como parte de pipeline CI
- Critério: Script que roda indexer como parte de pipeline CI
- Dependências: bash, cargo, git
- Validação: Executa build + test + index em loop

## Task 7: Docker Compose Example
- Fase: fase-4
- Arquivos: `examples/docker/docker-compose.yml`, `examples/docker/Dockerfile`
- Descrição: Compose file com indexer service rodando em container
- Critério: Compose file com indexer service
- Dependências: docker, docker-compose
- Validação: `docker-compose up` inicia container com indexer funcional

---

## Checklist de Validação

- [ ] Node.js basic executa e loga linguagens + chunks
- [ ] Node.js incremental demonstra Git diff
- [ ] Python basic funciona equivalente ao Node
- [ ] Python streaming processa eventos corretamente
- [ ] Go example compila e executa
- [ ] CI script executa sem erros
- [ ] Docker compose inicia corretamente