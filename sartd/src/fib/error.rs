use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("std::io::Error")]
    StdIoErr(#[from] std::io::Error),
    #[error("failed to communicate with rtnetlink: {}", e)]
    FailedToCommunicateWithNetlink {
        #[from]
        e: rtnetlink::Error,
    },
    #[error("already exists")]
    AlreadyExists,
    #[error("failed to get prefix")]
    FailedToGetPrefix,
    #[error("gateway not found")]
    GatewayNotFound,
    #[error("invalid ad value")]
    InvalidADValue,
    #[error("invalid protocol")]
    InvalidProtocol,
    #[error("invalid scope")]
    InvalidScope,
    #[error("invalid type")]
    InvalidType,
    #[error("invalid ip version")]
    InvalidIpVersion,
    #[error("invalid next hop flag")]
    InvalidNextHopFlag,
    #[error("failed to parse address")]
    FailedToParseAddress,
    #[error("destination not found")]
    DestinationNotFound,
    #[error("config error")]
    Config(#[from] ConfigError),
    #[error("failed to insert")]
    FailedToInsert,
    #[error("failed to remove")]
    FailedToRemove,
    #[error("failed to register")]
    FailedToRegister,
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
