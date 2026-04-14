# Docker Compose Example

Demonstra como rodar o rust_indexer dentro de um container Docker.

## Arquivos

- `Dockerfile` - Imagem com rust_indexer compilado
- `docker-compose.yml` - Compose para subir o serviço

## Como usar

### 1. Build e start

```bash
cd examples/docker
docker-compose up --build -d
```

### 3. Iniciar o indexer interativamente

Para testar o indexer diretamente no container:

```bash
docker exec -it rust_indexer sh
rust_indexer
```

Ou enviar comandos via stdin:

```bash
docker exec -i rust_indexer sh -c 'echo {"protocol_version":"1.0.0","type":"command","command":"list_languages"} | rust_indexer'
```

### 4. Montar volume com código para indexar

O compose já monta `../../src` em `/data/src`. Para indexar:

```bash
docker exec -i rust_indexer sh -c 'echo "{\"protocol_version\":\"1.0.0\",\"type\":\"command\",\"command\":\"index_path\",\"job_id\":\"job-1\",\"payload\":{\"path\":\"/data/src\"}}" | rust_indexer'
```
