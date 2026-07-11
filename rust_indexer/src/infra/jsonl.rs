use crate::{
    application::protocol::{ChunkEventPayload, Command, Event},
    domain::types::{CallEdge, ImportEdge},
};

pub fn write_event(e: &Event) {
    let s = serde_json::to_string(e).unwrap_or_else(|_| "{}".into());
    println!("{}", s);
}

pub fn build_chunk_event(job_id: Option<String>, payload: &ChunkEventPayload) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "chunk_emitted".into(),
        job_id,
        payload: Some(serde_json::to_value(payload).unwrap_or(serde_json::Value::Null)),
    }
}

pub fn write_chunk_event(job_id: Option<String>, payload: &ChunkEventPayload) {
    let ev = build_chunk_event(job_id, payload);
    write_event(&ev);
}

pub fn build_import_event(job_id: Option<String>, payload: &ImportEdge) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "import_edge".into(),
        job_id,
        payload: Some(serde_json::to_value(payload).unwrap_or(serde_json::Value::Null)),
    }
}

pub fn write_import_event(job_id: Option<String>, payload: &ImportEdge) {
    let ev = build_import_event(job_id, payload);
    write_event(&ev);
}

pub fn build_call_event(job_id: Option<String>, payload: &CallEdge) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "call_edge".into(),
        job_id,
        payload: Some(serde_json::to_value(payload).unwrap_or(serde_json::Value::Null)),
    }
}

pub fn write_call_event(job_id: Option<String>, payload: &CallEdge) {
    let ev = build_call_event(job_id, payload);
    write_event(&ev);
}

pub fn read_command(line: &str) -> Option<Command> {
    serde_json::from_str(line).ok()
}

// Backpressure events
pub fn build_pause_event(job_id: Option<String>) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "pause".into(),
        job_id,
        payload: None,
    }
}

pub fn write_pause_event(job_id: Option<String>) {
    let ev = build_pause_event(job_id);
    write_event(&ev);
}

pub fn build_resume_event(job_id: Option<String>) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "resume".into(),
        job_id,
        payload: None,
    }
}

pub fn write_resume_event(job_id: Option<String>) {
    let ev = build_resume_event(job_id);
    write_event(&ev);
}

// Backpressure events with payload
pub fn build_pause_event_with_payload(job_id: Option<String>, payload: serde_json::Value) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "pause".into(),
        job_id,
        payload: Some(payload),
    }
}

pub fn write_pause_event_with_payload(job_id: Option<String>, payload: serde_json::Value) {
    let ev = build_pause_event_with_payload(job_id, payload);
    write_event(&ev);
}

pub fn build_resume_event_with_payload(
    job_id: Option<String>,
    payload: serde_json::Value,
) -> Event {
    Event {
        protocol_version: "1.0.0".into(),
        r#type: "event".into(),
        event: "resume".into(),
        job_id,
        payload: Some(payload),
    }
}

pub fn write_resume_event_with_payload(job_id: Option<String>, payload: serde_json::Value) {
    let ev = build_resume_event_with_payload(job_id, payload);
    write_event(&ev);
}

/// Emite um evento com controle de backpressure.
///
/// Se o monitor estiver paused (fila cheia), a thread BLOQUEIA
/// via Condvar até que o consumidor envie ACK suficiente para
/// trazer a fila abaixo do limiar de resume.
pub fn emit_with_backpressure<F>(
    monitor: &crate::infra::backpressure::BackpressureMonitor,
    event_builder: F,
) -> Result<(), crate::infra::backpressure::BackpressureConfigError>
where
    F: FnOnce() -> Event,
{
    // Always increment queue first
    monitor.increment_queue_size();

    // Se paused, bloqueia até que o consumidor ACKe chunks
    // (Condvar notify via check_and_maybe_resume / check_timeout).
    if monitor.is_paused() {
        monitor.wait_until_resumed();
    } else {
        let current_size = monitor.current_queue_size();
        if current_size >= monitor.config().max_queue_size {
            monitor.check_and_maybe_pause();
            monitor.wait_until_resumed();
        }
    }

    // Emit the event
    let event = event_builder();
    write_event(&event);

    // After emission, check if we need to pause for NEXT event
    monitor.check_and_maybe_pause();

    Ok(())
}

/// Emite evento de chunk com controle de backpressure.
///
/// Esta é uma versão especializada de `emit_with_backpressure` para chunks.
///
/// # Arguments
///
/// * `monitor` — Referência ao monitor de backpressure
/// * `job_id` — ID do job
/// * `payload` — Payload do evento chunk
///
/// # Returns
///
/// Retorna `Ok(())` se o evento foi emitido ou `BackpressureConfigError` se houver erro.
pub fn emit_chunk_with_backpressure(
    monitor: &crate::infra::backpressure::BackpressureMonitor,
    job_id: Option<String>,
    payload: &ChunkEventPayload,
) -> Result<(), crate::infra::backpressure::BackpressureConfigError> {
    emit_with_backpressure(monitor, || build_chunk_event(job_id.clone(), payload))
}

/// Emite evento de import com controle de backpressure.
///
/// Esta é uma versão especializada de `emit_with_backpressure` para imports.
///
/// # Arguments
///
/// * `monitor` — Referência ao monitor de backpressure
/// * `job_id` — ID do job
/// * `payload` — Payload do evento import
///
/// # Returns
///
/// Retorna `Ok(())` se o evento foi emitido ou `BackpressureConfigError` se houver erro.
pub fn emit_import_with_backpressure(
    monitor: &crate::infra::backpressure::BackpressureMonitor,
    job_id: Option<String>,
    payload: &ImportEdge,
) -> Result<(), crate::infra::backpressure::BackpressureConfigError> {
    emit_with_backpressure(monitor, || build_import_event(job_id.clone(), payload))
}

/// Emite evento de call com controle de backpressure.
///
/// Esta é uma versão especializada de `emit_with_backpressure` para calls.
///
/// # Arguments
///
/// * `monitor` — Referência ao monitor de backpressure
/// * `job_id` — ID do job
/// * `payload` — Payload do evento call
///
/// # Returns
///
/// Retorna `Ok(())` se o evento foi emitido ou `BackpressureConfigError` se houver erro.
pub fn emit_call_with_backpressure(
    monitor: &crate::infra::backpressure::BackpressureMonitor,
    job_id: Option<String>,
    payload: &CallEdge,
) -> Result<(), crate::infra::backpressure::BackpressureConfigError> {
    emit_with_backpressure(monitor, || build_call_event(job_id.clone(), payload))
}

/// Emite evento de pause com controle de backpressure.
///
/// Esta é uma versão especializada de `emit_with_backpressure` para pauses.
///
/// # Arguments
///
/// * `monitor` — Referência ao monitor de backpressure
/// * `job_id` — ID do job
/// * `payload` — Payload opcional do evento pause (se None, usa evento sem payload)
///
/// # Returns
///
/// Retorna `Ok(())` se o evento foi emitido ou `BackpressureConfigError` se houver erro.
pub fn emit_pause_with_backpressure(
    monitor: &crate::infra::backpressure::BackpressureMonitor,
    job_id: Option<String>,
    payload: Option<serde_json::Value>,
) -> Result<(), crate::infra::backpressure::BackpressureConfigError> {
    emit_with_backpressure(monitor, || {
        if let Some(payload) = payload {
            build_pause_event_with_payload(job_id.clone(), payload)
        } else {
            build_pause_event(job_id.clone())
        }
    })
}

/// Emite evento de resume com controle de backpressure.
///
/// Esta é uma versão especializada de `emit_with_backpressure` para resumes.
///
/// # Arguments
///
/// * `monitor` — Referência ao monitor de backpressure
/// * `job_id` — ID do job
/// * `payload` — Payload opcional do evento resume (se None, usa evento sem payload)
///
/// # Returns
///
/// Retorna `Ok(())` se o evento foi emitido ou `BackpressureConfigError` se houver erro.
pub fn emit_resume_with_backpressure(
    monitor: &crate::infra::backpressure::BackpressureMonitor,
    job_id: Option<String>,
    payload: Option<serde_json::Value>,
) -> Result<(), crate::infra::backpressure::BackpressureConfigError> {
    emit_with_backpressure(monitor, || {
        if let Some(payload) = payload {
            build_resume_event_with_payload(job_id.clone(), payload)
        } else {
            build_resume_event(job_id.clone())
        }
    })
}

/// Emite qualquer evento genérico passando pelo controle de backpressure.
pub fn emit_event_with_backpressure(
    monitor: &crate::infra::backpressure::BackpressureMonitor,
    event: Event,
) -> Result<(), crate::infra::backpressure::BackpressureConfigError> {
    emit_with_backpressure(monitor, || event)
}

/// Emite qualquer evento genérico passando pelo controle de backpressure.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infra::backpressure::{BackpressureConfig, BackpressureMonitor};
    use std::sync::Arc;

    #[test]
    fn build_pause_event_contains_metadata() {
        let ev = build_pause_event(Some("job-1".into()));
        assert_eq!(ev.event, "pause");
        assert_eq!(ev.job_id.as_deref(), Some("job-1"));
        assert!(ev.payload.is_none());
    }

    #[test]
    fn build_resume_event_contains_metadata() {
        let ev = build_resume_event(Some("job-1".into()));
        assert_eq!(ev.event, "resume");
        assert_eq!(ev.job_id.as_deref(), Some("job-1"));
        assert!(ev.payload.is_none());
    }

    #[test]
    fn build_chunk_event_contains_payload_and_metadata() {
        let payload = ChunkEventPayload {
            chunk_id: "chunk-1".into(),
            chunk_kind: crate::application::protocol::ChunkKind::FullFile,
            file: "src/lib.rs".into(),
            language: Some("rust".into()),
            symbol_id: Some("sym-1".into()),
            start_line: 1,
            end_line: 10,
            text: "fn main() {}".into(),
            chunk_md5: "abc123".into(),
            size: 12,
        };

        let ev = build_chunk_event(Some("job-1".into()), &payload);
        assert_eq!(ev.event, "chunk_emitted");
        assert_eq!(ev.job_id.as_deref(), Some("job-1"));
        let val = ev.payload.unwrap();
        let expected = serde_json::to_value(&payload).unwrap();
        assert_eq!(val, expected);
    }

    #[test]
    fn test_emit_with_backpressure_when_not_paused_and_queue_below_threshold() {
        let config = BackpressureConfig::with_max_queue_size(100).unwrap();
        let monitor = BackpressureMonitor::new(config, 50, Some("test-job".to_string())).unwrap();

        let mut event_emitted = false;
        let result = emit_with_backpressure(&monitor, || {
            event_emitted = true;
            build_chunk_event(
                None,
                &ChunkEventPayload {
                    chunk_id: "test-chunk".into(),
                    chunk_kind: crate::application::protocol::ChunkKind::FullFile,
                    file: "test.rs".into(),
                    language: Some("rust".into()),
                    symbol_id: None,
                    start_line: 1,
                    end_line: 10,
                    text: "test".into(),
                    chunk_md5: "md5".into(),
                    size: 4,
                },
            )
        });

        assert!(result.is_ok());
        assert!(event_emitted);
        assert!(!monitor.is_paused());
    }

    #[test]
    fn test_emit_with_backpressure_blocks_when_paused_and_resumes_on_force() {
        let config = BackpressureConfig::with_max_queue_size(100).unwrap();
        let monitor = Arc::new(
            BackpressureMonitor::new(config, 100, Some("test-job".to_string())).unwrap(),
        );

        // Pre-pause via check_and_maybe_pause (counter=100 >= 100)
        monitor.check_and_maybe_pause();
        assert!(monitor.is_paused());

        let mon_clone = Arc::clone(&monitor);
        let handle = std::thread::spawn(move || {
            let mut emitted = false;
            let _ = emit_with_backpressure(&*mon_clone, || {
                emitted = true;
                build_chunk_event(
                    None,
                    &ChunkEventPayload {
                        chunk_id: "test-chunk".into(),
                        chunk_kind: crate::application::protocol::ChunkKind::FullFile,
                        file: "test.rs".into(),
                        language: Some("rust".into()),
                        symbol_id: None,
                        start_line: 1,
                        end_line: 10,
                        text: "test".into(),
                        chunk_md5: "md5".into(),
                        size: 4,
                    },
                )
            });
            emitted
        });

        // Give the thread time to block on the Condvar
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(monitor.is_paused());

        // Force resume — should unblock the thread
        monitor.force_resume();
        assert!(!monitor.is_paused());

        // The thread should now complete and have emitted the event
        let emitted = handle.join().unwrap();
        assert!(emitted, "blocked thread should emit after resume");
    }

    #[test]
    fn test_emit_chunk_with_backpressure() {
        let config = BackpressureConfig::with_max_queue_size(100).unwrap();
        let monitor = BackpressureMonitor::new(config, 50, Some("test-job".to_string())).unwrap();

        let payload = ChunkEventPayload {
            chunk_id: "test-chunk".into(),
            chunk_kind: crate::application::protocol::ChunkKind::FullFile,
            file: "test.rs".into(),
            language: Some("rust".into()),
            symbol_id: None,
            start_line: 1,
            end_line: 10,
            text: "test".into(),
            chunk_md5: "md5".into(),
            size: 4,
        };

        let result = emit_chunk_with_backpressure(&monitor, Some("test-job".to_string()), &payload);
        assert!(result.is_ok());
    }

    #[test]
    fn test_emit_import_with_backpressure() {
        let config = BackpressureConfig::with_max_queue_size(100).unwrap();
        let monitor = BackpressureMonitor::new(config, 50, Some("test-job".to_string())).unwrap();

        let payload = ImportEdge {
            id: "import-1".into(),
            from_file: "source.rs".into(),
            to_module: "target".into(),
            imported_symbol: Some("function".into()),
            alias: None,
            import_kind: "import".into(),
            location: crate::domain::types::Location {
                start_line: 42,
                start_col: 0,
                end_line: 42,
                end_col: 10,
            },
            resolved: true,
        };

        let result =
            emit_import_with_backpressure(&monitor, Some("test-job".to_string()), &payload);
        assert!(result.is_ok());
    }

    #[test]
    fn test_emit_call_with_backpressure() {
        let config = BackpressureConfig::with_max_queue_size(100).unwrap();
        let monitor = BackpressureMonitor::new(config, 50, Some("test-job".to_string())).unwrap();

        let payload = CallEdge {
            id: "call-1".into(),
            caller_symbol_id: Some("caller-symbol".into()),
            callee_name: "callee_function".into(),
            callee_symbol_id: Some("callee-symbol".into()),
            call_kind: "function_call".into(),
            location: crate::domain::types::Location {
                start_line: 42,
                start_col: 0,
                end_line: 42,
                end_col: 10,
            },
            resolved: true,
        };

        let result = emit_call_with_backpressure(&monitor, Some("test-job".to_string()), &payload);
        assert!(result.is_ok());
    }
}
