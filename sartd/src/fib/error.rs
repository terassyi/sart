use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("failed to communicate with rtnetlink")]
    FailedToCommunicateWithNetlink(#[from] rtnetlink::Error),
    #[error("already exists")]
    AlreadyExists,
    #[error("failed to get prefix")]
    FailedToGetPrefix,
    #[error("invalid ad value")]
    InvalidADValue,
    #[error("invalid protocol")]
    InvalidProtocol,
    #[error("failed to parse address")]
    FailedToParseAddress,
    #[error("destination not found")]
    DestinationNotFound,
}
