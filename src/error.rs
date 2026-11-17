use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArcsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Archive error: {0}")]
    Archive(String),

    #[error("Encryption error: {0}")]
    Crypto(String),

    #[error("Invalid password or corrupted archive")]
    AuthenticationFailed,

    #[error("Manifest parse error: {0}")]
    Manifest(String),

    #[error("Discovery error: {0}")]
    Discovery(String),

    #[error("Restore error: {0}")]
    Restore(String),

    #[error("Cloud error: {0}")]
    Cloud(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Unsupported on this platform: {0}")]
    Unsupported(String),

    #[error("Cancelled by user")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, ArcsError>;

impl From<anyhow::Error> for ArcsError {
    fn from(e: anyhow::Error) -> Self {
        ArcsError::Other(e.to_string())
    }
}

impl From<serde_json::Error> for ArcsError {
    fn from(e: serde_json::Error) -> Self {
        ArcsError::Manifest(e.to_string())
    }
}

impl From<serde_yaml::Error> for ArcsError {
    fn from(e: serde_yaml::Error) -> Self {
        ArcsError::Config(e.to_string())
    }
}

impl From<zip::result::ZipError> for ArcsError {
    fn from(e: zip::result::ZipError) -> Self {
        ArcsError::Archive(e.to_string())
    }
}

impl From<aes_gcm::Error> for ArcsError {
    fn from(_: aes_gcm::Error) -> Self {
        ArcsError::AuthenticationFailed
    }
}