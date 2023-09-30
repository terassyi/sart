use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("std::io::Error")]
    StdIoError(#[from] std::io::Error),

    #[error("config error")]
    ConfigError(#[from] ConfigError),
}

#[derive(Debug, Error)]
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
