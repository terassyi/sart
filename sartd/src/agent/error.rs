use thiserror::Error;

use crate::trace::error::TraceableError;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("std::io::Error")]
    StdIoError(#[from] std::io::Error),

    #[error("config error")]
    ConfigError(#[from] ConfigError),
}

#[derive(Debug, Error, Clone)]
pub(crate) enum ConfigError {
    #[error("already configured")]
    AlreadyConfigured,
    #[error("failed to load")]
    FailedToLoad,
    #[error("invalid argument")]
    InvalidArgument,
    #[error("invalid data")]
    InvalidData,
}

impl TraceableError for &Error {
    fn metric_label(&self) -> String {
        format!("{self:?}").to_lowercase()
    }
}
