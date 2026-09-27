use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("API error ({code}): {message}")]
    Api { code: String, message: String },

    #[error("Manifest error: {0}")]
    Manifest(String),

    #[error("Chunk error: {0}")]
    Chunk(String),

    #[error("Download error: {0}")]
    Download(String),

    #[error("Installation error: {0}")]
    Install(String),

    #[error("Verification failed: {0}")]
    Verification(String),

    #[error("Launch error: {0}")]
    Launch(String),

    #[error("Cloud save error: {0}")]
    CloudSave(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("General error: {0}")]
    General(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(err: reqwest::Error) -> Self {
        AppError::Network(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::Io(err.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::Database(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::General(format!("JSON error: {}", err))
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
