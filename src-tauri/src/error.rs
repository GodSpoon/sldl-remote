//! Application-wide error types.
//!
//! All fallible operations return `AppError`. This is converted to a string
//! for Tauri commands via `Result<T, String>`.

use std::fmt;

#[derive(Debug)]
pub enum AppError {
    /// Encryption/decryption failure.
    Crypto(String),
    /// Config file I/O or parse failure.
    Config(String),
    /// SSH remote command failure.
    Ssh { host: String, cmd: String, cause: String },
    /// Job not found in the in-memory registry.
    JobNotFound(String),
    /// Invalid user input (bad URL, unknown type, etc.).
    InvalidInput(String),
    /// Soulseek server rejected the connection (rate limit / ban).
    SoulseekBanned { ip: Option<String>, message: String },
    /// Internal state inconsistency.
    Internal(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Crypto(msg) => write!(f, "Crypto error: {}", msg),
            AppError::Config(msg) => write!(f, "Config error: {}", msg),
            AppError::Ssh { host, cmd, cause } => {
                write!(f, "SSH failed on {}: {} (cmd: {})", host, cause, cmd)
            }
            AppError::JobNotFound(id) => write!(f, "Job {} not found", id),
            AppError::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            AppError::SoulseekBanned { ip, message } => {
                write!(f, "Soulseek ban{}: {}", ip.as_ref().map(|i| format!(" (IP: {})", i)).unwrap_or_default(), message)
            }
            AppError::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Config(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Config(format!("JSON parse: {}", e))
    }
}

/// Convenience wrapper for command results.
pub type AppResult<T> = Result<T, AppError>;

/// Convert to the `Result<T, String>` shape Tauri expects.
pub fn to_tauri<T>(r: AppResult<T>) -> Result<T, String> {
    r.map_err(|e| e.to_string())
}
