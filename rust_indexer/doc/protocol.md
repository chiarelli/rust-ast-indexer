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
Emitted for each file discovered during scanning when running commands that stream results (for example: `index_path` when streaming, `list_files` / `dry_run` when the caller requests event streaming). The payload contains a `file` object (FileRecord) with stable metadata the caller can use for deduplication, ordering and to drive downstream parsing/indexing.

Payload fields:
- `file.path`: relative path to the repository root (string)
- `file.size`: file size in bytes (integer)
- `file.mtime`: modification time as seconds since UNIX epoch (integer)
- `file.hash`: deterministic content hash (blake3 hex) (string)
- `file.language`: detected language or null when unknown (string | null)

Example:
```json
{"protocol_version":"1.0.0","type":"event","event":"file_listed","job_id":"job-123","payload":{"file":{"path":"src/lib.rs","size":1234,"mtime":1610000000,"hash":"d41d8cd98f00b204e9800998ecf8427e","language":"rust"}}}
```

Note: callers may use `file.hash` (chunk/file hashes) to perform idempotent upserts into their storage and to skip unchanged files on incremental runs.

### `file_parsed`
```json
{"protocol_version":"1.0.0","type":"event","event":"file_parsed","job_id":"job-123","payload":{"file":"src/lib.rs","language":"rust","symbols":[{"id":"sym-1","kind":"function","name":"foo"}]}}
```

Note: When the optional parsing feature is enabled, the engine may emit `file_parsed` events containing extracted symbols and diagnostics. These events are produced by language adapters using Tree-sitter grammars and are not emitted when parsing is disabled.

### `chunk_emitted`
Emitted for each semantic chunk produced by the indexer. Chunks are the primary unit the caller will index and embed.

Payload fields:
- `chunk_id`: unique chunk id (string)
- `chunk_kind`: one of `FullFile` | `Symbol` | `Contextual` (string, PascalCase)
- `file`: relative path to repository root (string)
- `language`: detected language or null when unknown (string | null)
- `symbol_id`: optional symbol id this chunk is associated with (string | null)
- `start_line`: starting line number (1-based) included in the chunk (integer)
- `end_line`: ending line number (inclusive) (integer)
- `text`: textual content of the chunk (string)
- `chunk_md5`: md5 hash of the chunk text (string)
- `size`: size in bytes of the chunk text (integer)

Example:
```json
{"protocol_version":"1.0.0","type":"event","event":"chunk_emitted","job_id":"job-123","payload":{"chunk_id":"chunk-1","chunk_kind":"Symbol","file":"src/lib.rs","language":"rust","symbol_id":"sym-1","start_line":10,"end_line":40,"text":"fn foo() {}","chunk_md5":"d41d8cd98f00b204e9800998ecf8427e","size":12}}
```

Notes:
- `chunk_kind` semantics:
  - `FullFile`: the chunk represents an entire file (used for small files where the full file is relevant)
  - `Symbol`: the chunk maps to a symbol (function, method, class, etc.)
  - `Contextual`: a context window around code (used when symbol boundaries do not map cleanly)
- `chunk_md5` and `size` provide stable identifiers for caller-side deduplication and size-based batching for embedding.
- `text` MAY be truncated by the indexer based on configuration; callers should rely on `chunk_md5`/`size` for exact content checks.

### `job_progress`
```json
{"protocol_version":"1.0.0","type":"event","event":"job_progress","job_id":"job-123","payload":{"processed_files":10,"total_files":123,"queued_events":12,"is_paused":false}}
```

### `file_invalid`
Emitted when a file cannot be read or is detected as binary/non-UTF8. This event notifies the caller that the file was discovered but skipped and why. Emitted during streaming scans (e.g., `index_path` streaming) and by the walker callback when configured to emit events.

Common `reason` values:
- `non_utf8_or_binary` — file appears to be binary or contains NUL bytes / non-UTF8 sequences
- `io_error` — I/O error occurred when reading metadata or content
- `permission_denied` — permission error when accessing the file (may also be reported as `io_error`)

Example:
```json
{"protocol_version":"1.0.0","type":"event","event":"file_invalid","job_id":"job-123","payload":{"path":"assets/logo.bin","reason":"non_utf8_or_binary"}}
```

Note: Callers should treat `file_invalid` as recoverable per-file information (it does not cancel the whole job)."}]}]}]
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

## Eventos de Grafo — `import_edge` e `call_edge`

### `import_edge`

Emitido quando o adapter detecta um import/use/require durante o parsing.

**Schema:**
```json
{
  "type": "event",
  "event": "import_edge",
  "payload": {
    "id": "ie:<file>:<line>:<col>",
    "from_file": "src/lib.rs",
    "to_module": "std::collections",
    "imported_symbol": "HashMap",
    "alias": null,
    "import_kind": "named|default|namespace|side_effect|reexport",
    "location": { "start_line": 3, "start_col": 0, "end_line": 3, "end_col": 28 },
    "resolved": false
  }
}
```

**Exemplos por linguagem:**

Rust — `use std::collections::HashMap as Map;`
```json
{ "type": "event", "event": "import_edge", "payload": {
  "id": "ie:lib.rs:1:0", "from_file": "src/lib.rs",
  "to_module": "std::collections::HashMap",
  "imported_symbol": "HashMap", "alias": "Map",
  "import_kind": "named", "location": {"start_line":1,"start_col":0,"end_line":1,"end_col":40},
  "resolved": false
}}
```

Rust — `pub use` (reexport)
```json
{ "type": "event", "event": "import_edge", "payload": {
  "id": "ie:mod.rs:2:0", "from_file": "src/mod.rs",
  "to_module": "internal::helper", "import_kind": "reexport",
  "location": {"start_line":2,"start_col":0,"end_line":2,"end_col":30},
  "resolved": false
}}
```

TypeScript — `import { useState } from "react";`
```json
{ "type": "event", "event": "import_edge", "payload": {
  "id": "ie:app.tsx:1:0", "from_file": "src/app.tsx",
  "to_module": "react", "imported_symbol": "useState",
  "import_kind": "named",
  "location": {"start_line":1,"start_col":0,"end_line":1,"end_col":35},
  "resolved": false
}}
```

TypeScript — import relativo resolvível
```json
{ "type": "event", "event": "import_edge", "payload": {
  "id": "ie:app.tsx:3:0", "from_file": "src/app.tsx",
  "to_module": "./utils", "import_kind": "named",
  "location": {"start_line":3,"start_col":0,"end_line":3,"end_col":30},
  "resolved": true
}}
```

### `call_edge`

Emitido quando o adapter detecta uma chamada de função/método durante o parsing.

**Schema:**
```json
{
  "type": "event",
  "event": "call_edge",
  "payload": {
    "id": "ce:<file>:<line>:<col>",
    "caller_symbol_id": "sym:<file>:<caller_name>",
    "callee_name": "Parser::parse",
    "callee_symbol_id": null,
    "call_kind": "static|dynamic",
    "location": { "start_line": 120, "start_col": 8, "end_line": 120, "end_col": 20 },
    "resolved": false
  }
}
```

**Exemplo:**
```json
{ "type": "event", "event": "call_edge", "payload": {
  "id": "ce:lib.rs:120:8", "caller_symbol_id": "lib.rs:process",
  "callee_name": "format", "callee_symbol_id": null,
  "call_kind": "static",
  "location": {"start_line":120,"start_col":8,"end_line":120,"end_col":20},
  "resolved": false
}}
```

**Notas:**
- `resolved=false` na V1 — resolução de `callee_symbol_id` requer análise interprocedural futura.
- `call_kind=dynamic` para chamadas via index access (`a[b]()`) ou import dinâmico (`import("module")`).

## Eventos de Backpressure — `pause` e `resume`

### `pause`

Emitido automaticamente pela engine quando o queue size de output atinge `max_queue_size` configured no backpressure control.

**Schema:**
```json
{
  "type": "event",
  "event": "pause",
  "job_id": "<string>",
  "payload": {
    "reason": "output_queue_full|queue_near_capacity|external_signal",
    "threshold": <int>,
    "current_size": <int>,
    "backpressure_active": true
  }
}
```

**Campos do payload:**
| Campo | Tipo | Descrição |
|-------|------|-----------|
| `reason` | String | Motivo do `pause`: |
| | | - `output_queue_full`: size alcançou `max_queue_size` |
| | | - `queue_near_capacity`: size > 95% do threshold |
| | | - `external_signal`: comando externo solicitando pausa |
| `threshold` | Inteiro | Valor configurado de `max_queue_size` |
| `current_size` | Inteiro | Tamanho atual da fila de eventos |
| `backpressure_active` | Bool | Sempre `true` neste evento |

**Exemplo:**
```json
{ "type": "event", "event": "pause", "job_id": "job-123", "payload": {
  "reason": "output_queue_full",
  "threshold": 500,
  "current_size": 501,
  "backpressure_active": true
}}
```

### `resume`

Emitido automaticamente pela engine quando o queue size cai abaixo do `threshold_percent` (90% padrão do `max_queue_size`).

**Schema:**
```json
{
  "type": "event",
  "event": "resume",
  "job_id": "<string>",
  "payload": {
    "reason": "queue_under_threshold|external_signal",
    "threshold": <int>,
    "current_size": <int>,
    "backpressure_active": false
  }
}
```

**Campos do payload:**
| Campo | Tipo | Descrição |
|-------|------|-----------|
| `reason` | String | Motivo do `resume`: |
| | | - `queue_under_threshold`: size < 90% do `max_queue_size` |
| | | - `external_signal`: comando externo solicitando retomar |
| `threshold` | Inteiro | Valor 90% de `max_queue_size` onde resume é acionado |
| `current_size` | Inteiro | Tamanho atual da fila de eventos |
| `backpressure_active` | Bool | Sempre `false` neste evento |

**Exemplo:**
```json
{ "type": "event", "event": "resume", "job_id": "job-123", "payload": {
  "reason": "queue_under_threshold",
  "threshold": 450,
  "current_size": 449,
  "backpressure_active": false
}}
```

### Configuração de Backpressure na V1

Para configurar backpressure em comandos como `index_path`, inclua o objeto `backpressure` nas opções:

```json
{
  "protocol_version": "1.0.0",
  "type": "command",
  "command": "index_path",
  "seq": 1,
  "job_id": "job-123",
  "payload": {
    "path": "/repo",
    "language": "rust",
    "options": {
      "max_concurrency": 4,
      "backpressure": {
        "max_queue_size": 500,
        "threshold_percent": 90,
        "ack_required": false,
        "pause_timeout_secs": 300
      }
    }
  }
}
```

**Parâmetros de configuração:**

| Parâmetro | Tipo | Valor Padrão | Fa Válida | Descrição |
|-----------|------|--------------|-----------|-----------|
| `max_queue_size` | Inteiro | 500 | > 0 | Tamanho máximo da fila antes de triggers `pause` |
| `threshold_percent` | Inteiro | 90 | 80-99% | Porcentagem para triggers `resume` (exceção: 100% = imediatamente ao iniciar) |
| `ack_required` | Bool | `false` | - | Se `true`, consome deve ACK manualmente antes de `resume` ser eficaz |
| `pause_timeout_secs` | Inteiro | 300 | > 0 | Auto-resume após este tempo de `pause` sustentado |

### Fluxo de Backpressure

1. Engine monitora tamanho da output queue
2. Quando `size >= max_queue_size`:
   - Emite evento `pause` com `reason: output_queue_full`
   - Para de processar novos eventos
3. Quando `size < threshold_percent % of max_queue_size`:
   - Emite evento `resume` com `reason: queue_under_threshold`
   - Retoma processamento
4. Se `ack_required=true`, engine aguarda comando externo `resume` ou ACK explícito

### Semântica de `ack_required`

- `false` (padrão): `pause`/`resume` são notificações unidirecionais; engine decide automaticamente quando retomar
- `true`: engine requer confirmação explícita (comando `resume` ou ACK) antes de processar após `pause`

### Uso em CLI

O `pause` pode ser acionado via:

```json
{
  "command": "index_path",
  "payload": {
    "path": "/repo",
    "options": {
      "max_concurrency": 4,
      "backpressure": {
        "max_queue_size": 1000,
        "ack_required": false
      }
    }
  }
}
```

ou via `incremental_index` com opção equivalente.

### Observabilidade

- Um `resume` sem `pause` anterior pode indicar perda de eventos ou configuração excessivamente conservadora
- Logs de `pause`/`resume` devem ser correlacionados com timestamps para análise de performance
- Valores de `threshold_percent` muito baixos (80-85%) podem causar oscilações frequentes (flapping)
- Valores muito altos (95-99%) podem causar fila constantemente cheia

### Valores Recomendados

```
┌─────────────────────┬────────────────────────────┐
│ Configuração        │ Uso Recomendado            │
├─────────────────────┼────────────────────────────┤
│ max_queue_size: 500 │ Dev environments, baixo    │
│ threshold_percent:  │ tráfego                   │
│ 90                  │                            │
├─────────────────────┼────────────────────────────┤
│ max_queue_size: 2000│ Production, altoThroughput │
│ threshold_percent:  │                            │
│ 85                  │                            │
├─────────────────────┼────────────────────────────┤
│ max_queue_size: 5000│ Batch indexing, recursos   │
│ threshold_percent:  │ abundantes                │
│ 80                  │                            │
└─────────────────────┴────────────────────────────┘
```

## Armadilhas e boas práticas (achados de teste)

Esta seção documenta comportamentos não óbvios descobertos ao exercitar o
backpressure de ponta a ponta. Leia antes de integrar um caller.

### 1. `max_queue_size` tem mínimo de 10

`max_queue_size` **não aceita valores abaixo de 10** (`MIN_BACKPRESSURE_QUEUE_SIZE`).
Valores menores que o buffer do pipe do SO (~64KB) fazem o `BackpressureMonitor`
nunca observar a fila chegar ao limite — o pipe bloqueia primeiro e o `pause`
nunca é emitido, tornando o backpressure ineficaz.

Se você enviar um valor inválido (ex.: `max_queue_size: 5`), o indexer responde
com um evento `error` claro e **não trava**:

```json
{"protocol_version":"1.0.0","type":"event","event":"error","job_id":"job-x","payload":{"code":"BACKPRESSURE_CONFIG","message":"invalid backpressure config: tamanho de fila inválido: max_queue_size=5 é menor que o mínimo de 10.","recoverable":false}}
{"protocol_version":"1.0.0","type":"event","event":"job_completed","job_id":"job-x","payload":{"duration_ms":0,"errors":1,"processed":0}}
```

> **Histórico:** antes da correção, uma config inválida causava um `panic`
> silencioso dentro de uma thread do Rayon, que travava o processo sem emitir
> nenhum evento de erro. Agora o erro é propagado e reportado como
> `BACKPRESSURE_CONFIG`.

### 2. O caller DEVE drenar a fila com `ack`

O `pause` bloqueia a produção de novos eventos até que a fila caia abaixo do
threshold. Com `ack_required: false` (padrão), o resume é automático **somente
quando a fila é drenada** — e a fila só é drenada se o caller enviar comandos
`ack` com `count`.

**Fluxo correto do caller:**

1. Recebe evento `pause` (fila atingiu `max_queue_size`)
2. Envia comando `ack` com `count` para decrementar a fila
3. Recebe evento `resume` quando a fila cai abaixo do threshold
4. Continua lendo eventos até `job_completed`

```json
{"protocol_version":"1.0.0","type":"command","command":"ack","seq":2,"job_id":"job-x","payload":{"count":10}}
```

> **Atenção:** um caller que apenas lê a saída **sem** enviar `ack` fará o job
> ficar preso em `pause` para sempre (até o `pause_timeout_secs`). O backpressure
> é um protocolo de mão dupla: o produtor pausa, o consumidor confirma.

### 3. `file_listed` não passa pelo backpressure

Os eventos `file_listed` são emitidos durante o walk, **antes** do pipeline de
chunks, e **não** passam pelo controle de backpressure. Apenas `chunk_emitted`
(e imports/calls quando configurados) passam por `emit_with_backpressure`. Por
isso, com muitos arquivos você pode ver todos os `file_listed` antes de qualquer
`pause`.

### 4. Eventos terminais sempre chegam

`job_completed` e `error` são emitidos **diretamente**, ignorando o backpressure,
para que o caller sempre receba o fim do job — mesmo que a fila esteja cheia.

### 5. Observabilidade

- Um `resume` sem `pause` anterior pode indicar perda de eventos ou config
  excessivamente conservadora.
- `threshold_percent` muito baixo (80-85%) causa oscilações (flapping); muito
  alto (95-99%) mantém a fila constantemente cheia.
- Correlacione `pause`/`resume` com timestamps para análise de performance.


