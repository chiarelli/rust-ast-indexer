//! Controle de backpressure para o pipeline de indexação.
//!
//! Este módulo fornece configuração e monitoramento para gerenciar backpressure
//! quando a fila de saída atinge limites de capacidade.

use serde::{Deserialize, Serialize};

mod monitor;
pub use monitor::BackpressureMonitor;

/// Erros que podem ocorrer durante configuração de backpressure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackpressureConfigError {
    /// Tamanho da fila deve ser um valor positivo.
    InvalidQueueSize(String),

    /// Valor do limiar (80-99) é inválido.
    InvalidThreshold(String),

    /// Modo de ACK requer acknowledgment ativado.
    InvalidAckMode(String),
}

impl std::fmt::Display for BackpressureConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackpressureConfigError::InvalidQueueSize(msg) => {
                write!(f, "tamanho de fila inválido: {}", msg)
            }
            BackpressureConfigError::InvalidThreshold(msg) => {
                write!(f, "limiar inválido: {}", msg)
            }
            BackpressureConfigError::InvalidAckMode(msg) => {
                write!(f, "modo de acknowledgment inválido: {}", msg)
            }
        }
    }
}

impl std::error::Error for BackpressureConfigError {}

/// Razão para emitir evento de pause.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseReason {
    OutputQueueFull,
    QueueNearCapacity,
    ExternalSignal,
}

/// Razão para emitir evento de resume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeReason {
    QueueUnderThreshold,
    ExternalSignal,
}

/// Alias para JobId (já existe como Option<String> em protocol.rs)
pub type JobId = Option<String>;

/// Payload comum para eventos de pause e resume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PauseResumePayload {
    /// Razão do evento.
    #[serde(flatten)]
    pub reason: PauseResumeReason,
    /// Limite que foi atingido ou abaixo do qual resume é ativado.
    pub threshold: usize,
    /// Tamanho atual da fila.
    pub current_size: usize,
    /// Estado atual de backpressure.
    pub backpressure_active: bool,
}

/// Tipo para representar Either<PauseReason, ResumeReason>.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PauseResumeReason {
    Pause(PauseReason),
    Resume(ResumeReason),
}

impl PauseResumeReason {
    /// Retorna o nome do tipo (pause ou resume).
    pub fn kind(&self) -> &'static str {
        match self {
            PauseResumeReason::Pause(_) => "pause",
            PauseResumeReason::Resume(_) => "resume",
        }
    }
}

/// Configuração para controle de backpressure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackpressureConfig {
    /// Tamanho máximo da fila de saída antes de acionar backpressure.
    ///
    /// Padrão: 500 eventos
    #[serde(default = "default_max_queue_size")]
    pub max_queue_size: usize,

    /// Porcentagem de max_queue_size abaixo da qual resume é ativado.
    ///
    /// Intervalo: 80-99 (exclusivo).
    ///
    /// Padrão: 90
    #[serde(default = "default_threshold_percent")]
    pub threshold_percent: u8,

    /// Se o evento resume requer acknowledgment do consumidor.
    ///
    /// Se `true`, o consumidor deve confirmar explicitamente antes de processar resumes.
    /// Padrão: `false` (notificação unidirecional)
    #[serde(default = "default_ack_required")]
    pub ack_required: bool,

    /// Duração máxima do estado de pausa antes de recuperação automática.
    ///
    /// Padrão: 5 minutos
    #[serde(default = "default_pause_timeout")]
    pub pause_timeout_secs: u64,
}

fn default_max_queue_size() -> usize {
    500
}

fn default_threshold_percent() -> u8 {
    90
}

fn default_ack_required() -> bool {
    false
}

fn default_pause_timeout() -> u64 {
    300
}

impl Default for BackpressureConfig {
    fn default() -> Self {
        Self {
            max_queue_size: default_max_queue_size(),
            threshold_percent: default_threshold_percent(),
            ack_required: default_ack_required(),
            pause_timeout_secs: default_pause_timeout(),
        }
    }
}

impl BackpressureConfig {
    /// Cria uma nova `BackpressureConfig` com o tamanho máximo de fila dado.
    pub fn with_max_queue_size(size: usize) -> Result<Self, BackpressureConfigError> {
        if size == 0 {
            return Err(BackpressureConfigError::InvalidQueueSize(
                "tamanho de fila deve ser maior que zero".to_string(),
            ));
        }
        Ok(Self {
            max_queue_size: size,
            ..Self::default()
        })
    }

    /// Valida todos os campos da configuração.
    pub fn validate(&self) -> Result<(), BackpressureConfigError> {
        if self.max_queue_size == 0 {
            return Err(BackpressureConfigError::InvalidQueueSize(
                "max_queue_size deve ser positivo".to_string(),
            ));
        }

        if !(80..=99).contains(&self.threshold_percent) {
            return Err(BackpressureConfigError::InvalidThreshold(
                "threshold_percent deve estar entre 80 e 99".to_string(),
            ));
        }

        Ok(())
    }

    /// Calcula o limiar de resume baseado em max_queue_size e threshold_percent.
    pub fn resume_threshold(&self) -> usize {
        (self.max_queue_size * (self.threshold_percent as usize)) / 100
    }

    /// Verifica se o tamanho atual da fila deve acionar backpressure.
    pub fn should_pause(&self, current_size: usize) -> bool {
        current_size >= self.max_queue_size
    }

    /// Verifica se o tamanho atual da fila deve permitir retomar.
    pub fn should_resume(&self, current_size: usize) -> bool {
        current_size < self.resume_threshold()
    }

    /// Cria uma razão de pause baseada no estado da fila.
    pub fn pause_reason_for_size(&self, current_size: usize) -> PauseReason {
        if current_size >= self.max_queue_size {
            PauseReason::OutputQueueFull
        } else {
            PauseReason::QueueNearCapacity
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = BackpressureConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.max_queue_size, 500);
        assert_eq!(config.threshold_percent, 90);
        assert!(!config.ack_required);
        assert_eq!(config.pause_timeout_secs, 300);
    }

    #[test]
    fn custom_max_queue_size_works() {
        let config = BackpressureConfig::with_max_queue_size(1000).unwrap();
        assert_eq!(config.max_queue_size, 1000);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn invalid_max_queue_size_returns_error() {
        let result = BackpressureConfig::with_max_queue_size(0);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            BackpressureConfigError::InvalidQueueSize(_)
        ));
    }

    #[test]
    fn resume_threshold_calculated_correctly() {
        let config = BackpressureConfig::with_max_queue_size(1000).unwrap();
        assert_eq!(config.resume_threshold(), 900); // 90% of 1000

        let config_500 = BackpressureConfig::with_max_queue_size(500).unwrap();
        assert_eq!(config_500.resume_threshold(), 450); // 90% of 500
    }

    #[test]
    fn should_pause_when_at_or_above_max() {
        let config = BackpressureConfig::with_max_queue_size(500).unwrap();

        assert!(!config.should_pause(499));
        assert!(config.should_pause(500));
        assert!(config.should_pause(600));
    }

    #[test]
    fn should_resume_when_below_threshold() {
        let config = BackpressureConfig::with_max_queue_size(500).unwrap();
        let threshold = config.resume_threshold(); // 450

        assert!(!config.should_resume(threshold));
        assert!(config.should_resume(threshold - 1));
        assert!(config.should_resume(100));
        assert!(config.should_resume(0));
    }

    #[test]
    fn pause_reason_for_size_returns_output_queue_full_when_at_max() {
        let config = BackpressureConfig::with_max_queue_size(500).unwrap();

        assert_eq!(
            config.pause_reason_for_size(500),
            PauseReason::OutputQueueFull
        );
        assert_eq!(
            config.pause_reason_for_size(1000),
            PauseReason::OutputQueueFull
        );
    }

    #[test]
    fn pause_reason_for_size_returns_queue_near_capacity_when_below_max() {
        let config = BackpressureConfig::with_max_queue_size(500).unwrap();

        assert_eq!(
            config.pause_reason_for_size(499),
            PauseReason::QueueNearCapacity
        );
        assert_eq!(
            config.pause_reason_for_size(400),
            PauseReason::QueueNearCapacity
        );
    }

    #[test]
    fn threshold_percent_validation_rejects_below_80() {
        let config = BackpressureConfig {
            max_queue_size: 500,
            threshold_percent: 79,
            ack_required: false,
            pause_timeout_secs: 300,
        };
        assert!(matches!(
            config.validate(),
            Err(BackpressureConfigError::InvalidThreshold(_))
        ));
    }

    #[test]
    fn threshold_percent_validation_rejects_above_99() {
        let config = BackpressureConfig {
            max_queue_size: 500,
            threshold_percent: 100,
            ack_required: false,
            pause_timeout_secs: 300,
        };
        assert!(matches!(
            config.validate(),
            Err(BackpressureConfigError::InvalidThreshold(_))
        ));
    }

    #[test]
    fn threshold_percent_validation_accepts_bounds() {
        let config_80 = BackpressureConfig {
            max_queue_size: 500,
            threshold_percent: 80,
            ack_required: false,
            pause_timeout_secs: 300,
        };
        assert!(config_80.validate().is_ok());

        let config_99 = BackpressureConfig {
            max_queue_size: 500,
            threshold_percent: 99,
            ack_required: false,
            pause_timeout_secs: 300,
        };
        assert!(config_99.validate().is_ok());
    }
}
