use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TelemetryError {
    #[error("Failed to initialize telemetry: {0}")]
    Init(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct TelemetryConfig {
    pub log_dir: PathBuf,
    pub level: tracing::Level,
    pub json_output: bool,
    pub human_output: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            log_dir: PathBuf::from("logs"),
            level: tracing::Level::INFO,
            json_output: true,
            human_output: true,
        }
    }
}

pub fn init_telemetry(config: &TelemetryConfig) -> Result<(), TelemetryError> {
    use tracing_subscriber::{EnvFilter, fmt, prelude::*};

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(config.level.as_str()));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true))
        .try_init()
        .map_err(|e| TelemetryError::Init(e.to_string()))?;

    Ok(())
}
