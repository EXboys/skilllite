//! Crate-level error type for `skilllite-agent`.

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
    Executor(#[from] skilllite_executor::Error),

    #[error(transparent)]
    Evolution(#[from] skilllite_evolution::Error),

    #[error(transparent)]
    Fs(#[from] skilllite_fs::Error),

    #[error(transparent)]
    Sandbox(#[from] skilllite_sandbox::Error),

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

macro_rules! bail {
    ($($arg:tt)*) => {
        return ::core::result::Result::Err($crate::error::Error::validation(format!($($arg)*)))
    };
}
pub(crate) use bail;
