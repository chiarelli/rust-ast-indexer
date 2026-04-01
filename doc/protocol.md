# doc/protocol.md — Protocolo JSONL

## Visão geral
Protocolo JSONL (newline-delimited JSON) sobre stdio. Cada linha é uma mensagem JSON independente. O binário inicia emitindo `capabilities` e depois processa comandos enviados pelo caller Node.js.

## Campos comuns
- `protocol_version`: string semver, ex. `"1.0.0"`
- `type`: `"command" | "event" | "ack"`
- `seq`: integer opcional, monotônico por emissor
- `job_id`: string opcional, obrigatório em jobs longos
- `timestamp`: string RFC3339 opcional porém recomendada

## Comandos
### `list_languages`
```json
{"protocol_version":"1.0.0","type":"command","command":"list_languages","seq":1}
```

### `index_path`
```json
{"protocol_version":"1.0.0","type":"command","command":"index_path","seq":2,"job_id":"job-123","payload":{"path":"/proj","language":"rust","ignore_patterns":["target/**"],"options":{"max_concurrency":8,"chunk_lines":200,"backpressure":{"max_queue_size":500,"ack_required":false}}}}
```

### `list_files`
```json
{"protocol_version":"1.0.0","type":"command","command":"list_files","seq":3,"payload":{"path":"/proj","filters":{"language":"rust"}}}
```

### `dry_run`
```json
{"protocol_version":"1.0.0","type":"command","command":"dry_run","seq":4,"payload":{"path":"/proj","filters":{"language":"rust"}}}
```

### `incremental_index`
Aceita lista explícita de arquivos ou modo Git.
```json
{"protocol_version":"1.0.0","type":"command","command":"incremental_index","seq":5,"job_id":"job-124","payload":{"path":"/proj","use_git":true,"git_range":{"from":"HEAD~1","to":"HEAD"},"files":["src/lib.rs"],"options":{"max_concurrency":4}}}
```

### `status`
```json
{"protocol_version":"1.0.0","type":"command","command":"status","seq":6,"job_id":"job-123"}
```

### `resume`
Usado quando o job entrou em pausa por backpressure.
```json
{"protocol_version":"1.0.0","type":"command","command":"resume","seq":7,"job_id":"job-123"}
```

### `cancel_job`
```json
{"protocol_version":"1.0.0","type":"command","command":"cancel_job","seq":8,"job_id":"job-123"}
```

### `stop`
```json
{"protocol_version":"1.0.0","type":"command","command":"stop","seq":9}
```

## Eventos
### `capabilities`
Emitido no startup.
```json
{"protocol_version":"1.0.0","type":"event","event":"capabilities","payload":{"version":"0.1.0","languages":["rust","go","python","typescript","javascript","java"],"features":["jsonl","incremental_index","git_diff","pause_resume","mcp_compatible"]}}
```

### `job_started`
```json
{"protocol_version":"1.0.0","type":"event","event":"job_started","job_id":"job-123","payload":{"total_files":123}}
```

### `file_listed`
```json
{"protocol_version":"1.0.0","type":"event","event":"file_listed","job_id":"job-123","payload":{"file":{"path":"src/lib.rs","size":1234,"hash":"abc"}}}
```

### `file_parsed`
```json
{"protocol_version":"1.0.0","type":"event","event":"file_parsed","job_id":"job-123","payload":{"file":"src/lib.rs","language":"rust","symbols":[{"id":"sym-1","kind":"function","name":"foo"}]}}
```

### `chunk_emitted`
```json
{"protocol_version":"1.0.0","type":"event","event":"chunk_emitted","job_id":"job-123","payload":{"chunk_id":"chunk-1","chunk_kind":"Symbol","file":"src/lib.rs","language":"rust","symbol_id":"sym-1","start_line":10,"end_line":40,"text":"fn foo() {}","chunk_md5":"d41d8cd98f00b204e9800998ecf8427e","size":12}}
```

### `job_progress`
```json
{"protocol_version":"1.0.0","type":"event","event":"job_progress","job_id":"job-123","payload":{"processed_files":10,"total_files":123,"queued_events":12,"is_paused":false}}
```

### `file_invalid`
Para arquivos binários ou não-UTF8.
```json
{"protocol_version":"1.0.0","type":"event","event":"file_invalid","job_id":"job-123","payload":{"path":"assets/logo.bin","reason":"non_utf8_or_binary"}}
```

### `error`
```json
{"protocol_version":"1.0.0","type":"event","event":"error","job_id":"job-123","payload":{"code":"BACKPRESSURE","message":"output queue is full","recoverable":true,"detail":{"pause_required":true}}}
```

### `job_completed`
```json
{"protocol_version":"1.0.0","type":"event","event":"job_completed","job_id":"job-123","payload":{"processed":123,"duration_ms":10000,"invalid_files":2,"errors":1}}
```

## ACK / Backpressure
- Modelo principal: pausa e resume.
- Config padrão: `max_queue_size=500`, `ack_required=false`.
- Se a fila encher, a engine:
  - emite `error` com `code=BACKPRESSURE`
  - marca o job como pausado
  - interrompe emissão de novos eventos até receber `resume`
- `ack` continua reservado para compatibilidade futura, mas a V1 usa `resume` como mecanismo operacional principal.

Exemplo de ack compatível:
```json
{"protocol_version":"1.0.0","type":"ack","seq":10,"job_id":"job-123","payload":{"ack_for_seq":45}}
```

## Status
`status` retorna algo como:
```json
{"protocol_version":"1.0.0","type":"event","event":"status","job_id":"job-123","payload":{"state":"running","processed_files":10,"total_files":123,"queued_events":12,"is_paused":false,"errors":0}}
```

## Códigos de erro recomendados
- `PARSER_FAIL`
- `IO_ERR`
- `INVALID_COMMAND`
- `BACKPRESSURE`
- `CANCELLED`
- `INTERNAL_ERR`
- `FILE_INVALID`

Todos os erros devem carregar:
- `code`
- `message`
- `recoverable`
- `file_path` opcional
- `detail` opcional

## Sequenciamento e idempotência
- `job_id` identifica um job de longa duração.
- `seq` ajuda no tracing e em compatibilidade futura com ACK explícito.
- O caller pode usar `chunk_md5` e hashes de arquivo para deduplicação idempotente no storage próprio.

## Robustez
- Linha JSON máxima recomendada: `1MB`.
- Comandos inválidos devem gerar `error` estruturado.
- O caller deve consumir stdout em streaming e nunca assumir que um chunk chega inteiro fora do delimitador de linha.

## Compatibilidade MCP
O protocolo é JSONL nativo, mas foi modelado para adaptação simples a MCP stdio. A V1 não exige conformidade MCP completa; um adapter poderá traduzir comandos e eventos no futuro.
