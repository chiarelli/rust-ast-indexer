use std::sync::{
    atomic::{AtomicBool, AtomicUsize},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};

use crate::{
    application::protocol::Event,
    infra::{
        backpressure::{
            BackpressureConfig, BackpressureConfigError, JobId, PauseResumePayload,
            PauseResumeReason, ResumeReason,
        },
        jsonl,
    },
};

/// Monitor de controle de backpressure para o pipeline.
///
/// Gerencia o estado atual de fila e emite eventos de pause/resume
/// com base no tamanho da fila e na configuração.
#[derive(Debug)]
pub struct BackpressureMonitor {
    config: BackpressureConfig,
    queue_size: Arc<AtomicUsize>,
    paused: Arc<AtomicBool>,
    paused_since: Arc<Mutex<Option<Instant>>>,
    pause_mtx: Mutex<()>,
    pause_cv: Condvar,
    job_id: JobId,
}

impl BackpressureMonitor {
    /// Cria um novo `BackpressureMonitor` com a configuração fornecida.
    ///
    /// # Arguments
    ///
    /// * `config` — Configuração de backpressure a ser usada
    /// * `initial_size` — Tamanho inicial da fila
    /// * `job_id` — ID do job para os eventos de pause/resume
    ///
    /// # Returns
    ///
    /// Retorna `Result<Self>` com erro se a configuração for inválida.
    pub fn new(
        config: BackpressureConfig,
        initial_size: usize,
        job_id: JobId,
    ) -> Result<Self, BackpressureConfigError> {
        config.validate()?;
        Ok(Self {
            config,
            queue_size: Arc::new(AtomicUsize::new(initial_size)),
            paused: Arc::new(AtomicBool::new(false)),
            paused_since: Arc::new(Mutex::new(None)),
            pause_mtx: Mutex::new(()),
            pause_cv: Condvar::new(),
            job_id,
        })
    }

    /// Cria um novo monitor com configuração padrão.
    pub fn with_default_config(
        initial_size: usize,
        job_id: JobId,
    ) -> Result<Self, BackpressureConfigError> {
        Self::new(BackpressureConfig::default(), initial_size, job_id)
    }

    /// Atualiza o tamanho atual da fila.
    pub fn set_queue_size(&self, size: usize) {
        self.queue_size
            .store(size, std::sync::atomic::Ordering::SeqCst);
    }

    /// Retorna o tamanho atual da fila.
    pub fn current_queue_size(&self) -> usize {
        self.queue_size.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Retorna o estado atual de pausa (true se backpressure está ativo).
    pub fn is_paused(&self) -> bool {
        self.paused.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Bloqueia a thread atual até que o estado paused seja desfeito
    /// (via ACK do consumidor ou timeout de pausa).
    ///
    /// Usa Condvar com timeout de 1s para permitir verificação periódica
    /// do timeout de pausa. Retorna quando o estado paused é desfeito.
    pub fn wait_until_resumed(&self) {
        let mut guard = self.pause_mtx.lock().unwrap();
        while self.is_paused() {
            guard = self
                .pause_cv
                .wait_timeout(guard, Duration::from_secs(1))
                .unwrap()
                .0;
            // A cada 1s verifica se o timeout de pausa expirou
            self.check_timeout();
        }
    }

    /// Notifica todas as threads bloqueadas em wait_until_resumed.
    ///
    /// A notificação é feita SEM lock do pause_mtx para evitar que
    /// os waiters acordem e tentem re-adquirir o lock
    /// enquanto o notifier ainda o segura (thundering herd).
    fn notify_waiters(&self) {
        self.pause_cv.notify_all();
    }

    /// Verifica se deve emitir evento de pause e atualiza o estado.
    ///
    /// Retorna `true` se o evento foi emitido.
    pub fn check_and_maybe_pause(&self) -> bool {
        let size = self.current_queue_size();

        if self.config.should_pause(size) && !self.is_paused() {
            let reason = self.config.pause_reason_for_size(size);
            let payload = PauseResumePayload {
                reason: PauseResumeReason::Pause(reason),
                threshold: self.config.max_queue_size,
                current_size: size,
                backpressure_active: true,
            };
            let event = Event {
                protocol_version: "1.0.0".into(),
                r#type: "event".into(),
                event: "pause".into(),
                job_id: self.job_id.clone(),
                payload: serde_json::to_value(&payload).ok(),
            };
            jsonl::write_event(&event);
            self.paused.store(true, std::sync::atomic::Ordering::SeqCst);
            *self.paused_since.lock().unwrap() = Some(Instant::now());
            true
        } else {
            false
        }
    }

    /// Verifica se deve emitir evento de resume e atualiza o estado.
    ///
    /// Retorna `true` se o evento foi emitido.
    pub fn check_and_maybe_resume(&self) -> bool {
        self.check_and_maybe_resume_with_reason(ResumeReason::QueueUnderThreshold)
    }

    /// Verifica se deve emitir evento de resume com uma razão específica.
    ///
    /// Retorna `true` se o evento foi emitido.
    pub fn check_and_maybe_resume_with_reason(&self, reason: ResumeReason) -> bool {
        let size = self.current_queue_size();
        let is_paused = self.is_paused();
        let should_resume = self.config.should_resume(size);

        if is_paused && should_resume {
            let payload = PauseResumePayload {
                reason: PauseResumeReason::Resume(reason),
                threshold: self.config.resume_threshold(),
                current_size: size,
                backpressure_active: false,
            };
            let event = Event {
                protocol_version: "1.0.0".into(),
                r#type: "event".into(),
                event: "resume".into(),
                job_id: self.job_id.clone(),
                payload: serde_json::to_value(&payload).ok(),
            };
            jsonl::write_event(&event);
            self.paused
                .store(false, std::sync::atomic::Ordering::SeqCst);
            *self.paused_since.lock().unwrap() = None;
            self.notify_waiters();
            true
        } else {
            false
        }
    }

    /// Verifica se o tempo de pausa excedeu o limite configurado.
    ///
    /// Se excedido, emite evento de resume com razão "pause_timeout" e reseta a fila.
    /// Retorna `true` se o timeout foi acionado e resume foi emitido.
    pub fn check_timeout(&self) -> bool {
        let is_paused = self.is_paused();
        if !is_paused {
            return false;
        }

        let paused_since = self.paused_since.lock().unwrap();
        if let Some(instant) = *paused_since {
            let elapsed = instant.elapsed().as_secs();
            if elapsed >= self.config.pause_timeout_secs {
                drop(paused_since);
                self.reset_queue();
                self.paused
                    .store(false, std::sync::atomic::Ordering::SeqCst);
                *self.paused_since.lock().unwrap() = None;
                let payload = PauseResumePayload {
                    reason: PauseResumeReason::Resume(ResumeReason::PauseTimeout),
                    threshold: self.config.resume_threshold(),
                    current_size: 0,
                    backpressure_active: false,
                };
                let event = Event {
                    protocol_version: "1.0.0".into(),
                    r#type: "event".into(),
                    event: "resume".into(),
                    job_id: self.job_id.clone(),
                    payload: serde_json::to_value(&payload).ok(),
                };
                jsonl::write_event(&event);
                self.notify_waiters();
                return true;
            }
        }
        false
    }

    /// Verifica e emite eventos apropriados (pause ou resume) se necessário.
    /// Também verifica timeout para pausas longas.
    pub fn check_and_emit(&self) -> (bool, bool) {
        let timeout_triggered = self.check_timeout();
        (
            self.check_and_maybe_pause(),
            self.check_and_maybe_resume() || timeout_triggered,
        )
    }

    /// Retorna a configuração deste monitor.
    pub fn config(&self) -> &BackpressureConfig {
        &self.config
    }

    /// Incrementa o tamanho da fila em 1.
    pub fn increment_queue_size(&self) {
        self.queue_size
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    /// Decrementa o tamanho da fila em N unidades.
    pub fn decrement_queue_size(&self, count: usize) {
        let _ = self.queue_size.fetch_update(
            std::sync::atomic::Ordering::SeqCst,
            std::sync::atomic::Ordering::SeqCst,
            |current| Some(current.saturating_sub(count)),
        );
    }

    /// Reseta a fila para tamanho zero.
    pub fn reset_queue(&self) {
        self.queue_size
            .store(0, std::sync::atomic::Ordering::SeqCst);
    }

    /// Força saída do estado pausado imediatamente.
    pub fn force_resume(&self) {
        self.reset_queue();
        self.paused
            .store(false, std::sync::atomic::Ordering::SeqCst);
        self.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // BackpressureMonitor tests

    #[test]
    fn monitor_initializes_with_given_config_and_job_id() {
        let config = BackpressureConfig::with_max_queue_size(100).unwrap();
        let job_id = Some("test-job-1".to_string());
        let monitor = BackpressureMonitor::new(config.clone(), 0, job_id.clone()).unwrap();

        assert_eq!(monitor.current_queue_size(), 0);
        assert!(!monitor.is_paused());
        assert_eq!(monitor.config().max_queue_size, 100);
        assert_eq!(monitor.job_id, job_id);
    }

    #[test]
    fn monitor_initializes_with_default_config() {
        let job_id = Some("test-job-2".to_string());
        let monitor = BackpressureMonitor::with_default_config(0, job_id.clone()).unwrap();

        assert_eq!(monitor.current_queue_size(), 0);
        assert!(!monitor.is_paused());
        assert_eq!(monitor.config().max_queue_size, 500);
        assert_eq!(monitor.job_id, job_id);
    }

    #[test]
    fn monitor_fails_to_initialize_with_invalid_config() {
        let invalid_config = BackpressureConfig {
            max_queue_size: 0,
            threshold_percent: 90,
            ack_required: false,
            pause_timeout_secs: 300,
        };
        let job_id = Some("test-job-invalid".to_string());
        let result = BackpressureMonitor::new(invalid_config, 0, job_id);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BackpressureConfigError::InvalidQueueSize(_)
        ));
    }
}
