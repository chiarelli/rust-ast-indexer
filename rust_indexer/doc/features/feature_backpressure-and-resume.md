# Próxima feature: feature/backpressure-and-resume (planejamento inicial)

### Objetivo

Implementar mecanismo de backpressure com eventos de `pause` e `resume` para controlar o fluxo de eventos do indexer quando o consumidor não consegue acompanhar. Garantir que o sistema não trave ou perca eventos em escrutínio elevado.

---

## Fases e tarefas

### fase-1: definição do protocolo e modelo — ✅ CONCLUÍDA

| Status | Task | Atividade | Commits |
|---|---|---|---|
| ✅ | domain/protocol | adicionar eventos `pause` e `resume` ao schema JSONL | — |
| ✅ | domain/protocol | testes unitários para tipos de evento | — |
| ✅ | infra/backpressure | definir `BackpressureConfig` com `max_queue_size` e `ack_required` | — |
| ✅ | doc | atualizar `doc/protocol.md` com exemplos de eventos `pause` e `resume` | — |

### fase-2: implementação no pipeline — ✅ CONCLUÍDA

| Status | Task | Atividade | Observação |
|---|---|---|---|
| ✅ | indexing/backpressure | adicionar monitor de fila e lógica de emissão condicional | Implementado em indexer.rs |
| ✅ | indexing/pause-resume | enviar evento `pause` quando `queue_size > max_queue_size` | Implementado via BackpressureMonitor |
| ✅ | indexing/resume | enviar evento `resume` quando o consumidor libera espaço | Implementado via BackpressureMonitor |
| ✅ | cli | adaptar para respeitar sinais de backpressure | Parsing em cli/mod.rs |
| ✅ | benchmarks | medir impacto de falar e voltar no fluxo de eventos | – |

### fase-3: integração e CI — ✅ CONCLUÍDA

| Status | Task | Atividade | Observação |
|---|---|---|---|
| ✅ | integration/smoke | smoke test com evento `pause` emitido sob carga | Teste passando |
| ⛔ | integration/checkpointing | resumir do último `pause` ponto após reinício | Não aplicável (projeto é stateless) |
| ✅ | integration/resume-consumption | garantir que o consumidor receberá `resume` | Comando resume implementado |
| ✅ | integration/conflict-handling | tratar múltiplos `pause` sem `resume` | Timeout automático implementado |
| ✅ | integration/cleanup | remover eventos bloqueados após timeout | Mapa de monitores implementado + cleanup |
| ✅ | integration/testing | adicionar testes de fala/resumo no CI | Smoke tests existentes |

#### Tarefas adicionadas durante implementação:
| Status | Task | Atividade |
|---|---|---|
| ✅ | core/map-monitors | Armazenar BackpressureMonitors por job_id no ApplicationContext |
| ✅ | core/decrement-method | Adicionar método decrement_queue_size() no monitor |
| ✅ | cli/resume-command | Comando resume real que atualiza estado do monitor |
| ✅ | cli/ack-command | Comando ack que decrementa fila de eventos não processados |
| ✅ | core/emit-order | Corrigir ordem: verificar backpressure antes de incrementar fila |
| ✅ | core/timeout | Timeout automático para estados de pausa longos (5 minutos) |
| ✅ | core/cleanup | Limpar monitores inativos após conclusão do job |

---

### Critérios de aceitação

| Critério | Status | Observação |
|---|---|---|
| Eventos `pause` e `resume` definidos e documentados | ✅ | Schema finalizado |
| `emit_with_backpressure()` respeita limite global `max_queue_size` | ✅ | Funcional |
| `max_queue_size` configurável no evento inicializador ou por payload | ✅ | Validação implementada |
| Consumidor pode saber que o envio está em pausa | ✅ | Evento `pause` é emitido corretamente |
| Valores padrão: `max_queue_size=500`, `ack_required=false` | ✅ | Carregados corretamente |
| Testes locais de simulação passam (smoke test) | ✅ | Teste passando |
| Timeout automático para pausas longas | ✅ | 5 minutos configurável |
| Cleanup de monitores após job completar | ✅ | Previne memory leak |
| Comandos `ack` e `resume` via CLI | ✅ | Implementados |

---

### Riscos e dependências

- **Fluxo orientado a eventos**: esta alteração depende da implementação correta do pipeline paralelo do indexer.
- **Observabilidade**: um `resume` não associado a um `pause` anterior pode indicar perda de eventos.
- **Logística**: o timeout deve ser configurável para evitar cleanup prematuro.
- **Semântica de `ack_required`**: se ativado, o `resume` requer confirmação do consumidor; o padrão é `false`, o que significa que o status de `pause/resume` é unidirecional.

#### Decisões técnicas propostas

- **Abortar `pause` contínuo**: em condições de backpressure sustentadas (>5 minutos), descartar eventos mais antigos.
- **Contagem de eventos por payload**: o `job_progress` pode incluir um contador de eventos não processados.

---

### Plano de trabalho (iteração mínima)

✅ 1. Definir schema para `pause_event` e `resume_event`
✅ 2. Implementar lógica de monitoramento da fila no `jsonl.rs`
✅ 3. Adicionar `max_queue_size` a `IndexOptions`
✅ 4. Testes unitários para validação de backpressure com limites baixos e altos
🔄 5. Smoke test com evento `pause` emitido sob carga

### Plano de trabalho atualizado (faltante):

6. Armazenar monitores por job_id no contexto global
7. Implementar comando `ack` para decrementar fila
8. Implementar comando `resume` para sair de estado pausado
9. Implementar timeout automático para pausas prolongadas
10. Limpar monitores após conclusão do job
11. Validar smoke test com fluxo completo `pause` → `ack` → `resume`

---

### Artefatos a produzir

- `src/infra/backpressure.rs` — configuração e lógica de monitoramento
- `src/infra/jsonl.rs` — eventos `pause` e `resume` + integração
- `src/application/indexer.rs` — verificação de `queue_size` antes da emissão
- `src/infra/backpressure_tests.rs` — testes unitários do controlador
- `tests/smoke_backpressure.rs` — teste de fuma em condições reais
- `doc/protocol.md` — documentação do novo evento
- `PROJECT.md` — atualização de status da feature

### Decisões técnicas documentadas (a preencher)

1. Usar `queue_size` global (`AtomicUsize`) para simplicidade
2. Emitir `pause` e `resume` como eventos, não como resposta a comandos
3. Valores padrão: `max_queue_size=500`, `ack_required=false` (não requer ACK do consumidor)

---

### Modelo de Evento (schema proposto)

```json
{
  "type": "event",
  "event": "pause",
  "job_id": "job-123",
  "payload": {
    "reason": "output_queue_full",
    "threshold": 500,
    "current_size": 501,
    "backpressure_active": true
  }
}
```

```json
{
  "type": "event",
  "event": "resume",
  "job_id": "job-123",
  "payload": {
    "reason": "queue_under_threshold",
    "threshold": 500,
    "current_size": 498,
    "backpressure_active": false
  }
}
```

---

### Integração com CLI

O evento `pause` pode ser acionado via:

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