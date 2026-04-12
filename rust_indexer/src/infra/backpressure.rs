//! Backpressure control for the indexing pipeline.
//!
//! This module provides configuration and utilities for managing backpressure
//! when the output queue reaches capacity limits.

use serde::{Deserialize, Serialize};

/// Errors that can occur during backpressure configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackpressureConfigError {
    /// Queue size must be a positive value.
    InvalidQueueSize(String),

    /// Threshold value (80-99) is invalid.
    InvalidThreshold(String),

    /// Acknowledgment mode requires acknowledgment to be enabled.
    InvalidAckMode(String),
}

impl std::fmt::Display for BackpressureConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackpressureConfigError::InvalidQueueSize(msg) => {
                write!(f, "invalid queue size: {}", msg)
            }
            BackpressureConfigError::InvalidThreshold(msg) => {
                write!(f, "invalid threshold: {}", msg)
            }
            BackpressureConfigError::InvalidAckMode(msg) => {
                write!(f, "invalid acknowledgment mode: {}", msg)
            }
        }
    }
}

impl std::error::Error for BackpressureConfigError {}

/// Reason for emitting a pause event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PauseReason {
    OutputQueueFull,
    QueueNearCapacity,
    ExternalSignal,
}

/// Reason for emitting a resume event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResumeReason {
    QueueUnderThreshold,
    ExternalSignal,
}

/// Configuration for backpressure control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackpressureConfig {
    /// Maximum size of the output queue before triggering backpressure.
    ///
    /// Default: 500 events
    #[serde(default = "default_max_queue_size")]
    pub max_queue_size: usize,

    /// Percentage of max_queue_size below which resume is triggered.
    ///
    /// Range: 80-99 (exclusive).
    ///
    /// Default: 90
    #[serde(default = "default_threshold_percent")]
    pub threshold_percent: u8,

    /// Whether the resume event requires acknowledgment from the consumer.
    ///
    /// If `true`, the consumer must explicitly acknowledge before processing resumes.
    /// Default: `false` (unidirectional notification)
    #[serde(default = "default_ack_required")]
    pub ack_required: bool,

    /// Maximum duration of pause state before automatic recovery.
    ///
    /// Default: 5 minutes
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
    /// Creates a new `BackpressureConfig` with the given maximum queue size.
    pub fn with_max_queue_size(size: usize) -> Result<Self, BackpressureConfigError> {
        if size == 0 {
            return Err(BackpressureConfigError::InvalidQueueSize(
                "queue size must be greater than zero".to_string(),
            ));
        }
        Ok(Self {
            max_queue_size: size,
            ..Self::default()
        })
    }

    /// Validates all fields of the configuration.
    pub fn validate(&self) -> Result<(), BackpressureConfigError> {
        if self.max_queue_size == 0 {
            return Err(BackpressureConfigError::InvalidQueueSize(
                "max_queue_size must be positive".to_string(),
            ));
        }

        if !(80..=99).contains(&self.threshold_percent) {
            return Err(BackpressureConfigError::InvalidThreshold(
                "threshold_percent must be between 80 and 99".to_string(),
            ));
        }

        Ok(())
    }

    /// Calculates the resume threshold based on max_queue_size and threshold_percent.
    pub fn resume_threshold(&self) -> usize {
        (self.max_queue_size * (self.threshold_percent as usize)) / 100
    }

    /// Checks if the current queue size should trigger backpressure.
    pub fn should_pause(&self, current_size: usize) -> bool {
        current_size >= self.max_queue_size
    }

    /// Checks if the current queue size should allow resuming.
    pub fn should_resume(&self, current_size: usize) -> bool {
        current_size < self.resume_threshold()
    }

    /// Creates a pause reason based on queue state.
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
