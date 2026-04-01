//! Crate-level error type for `skilllite-commands`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Core(#[from] skilllite_core::Error),

    #[error(transparent)]
    Sandbox(#[from] skilllite_sandbox::Error),

    #[error(transparent)]
    Fs(#[from] skilllite_fs::Error),

    #[error(transparent)]
    Evolution(#[from] skilllite_evolution::Error),

    #[cfg(feature = "agent")]
    #[error(transparent)]
    Agent(#[from] skilllite_agent::Error),

    #[error("{0}")]
    Validation(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn validation(msg: impl Into<String>) -> Self {
        Error::Validation(msg.into())
    }
}

impl From<skilllite_core::error::PathValidationError> for Error {
    fn from(e: skilllite_core::error::PathValidationError) -> Self {
        Error::Validation(e.to_string())
    }
}

impl From<skilllite_sandbox::bash_validator::BashValidationError> for Error {
    fn from(e: skilllite_sandbox::bash_validator::BashValidationError) -> Self {
        Error::Validation(e.to_string())
    }
}

macro_rules! bail {
    ($($arg:tt)*) => {
        return ::core::result::Result::Err($crate::error::Error::validation(format!($($arg)*)))
    };
}
pub(crate) use bail;
