use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("std::io::Error")]
    StdIoErr(#[from] std::io::Error),
    #[error("failed to communicate with rtnetlink: {}", e)]
    FailedToCommunicateWithNetlink {
        #[from]
        e: rtnetlink::Error,
    },
    #[error("failed to communicate with gRPC server/client")]
    FailedToCommunicateWithgRPC(#[from] tonic::transport::Error),
    #[error("timeout")]
    Timeout,
    #[error("got error {} from gRPC", e)]
    GotgPRCError {
        #[from]
        e: tonic::Status,
    },
    #[error("failed to recv/send via channel")]
    FailedToRecvSendViaChannel,
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
    #[error("multi path is not equal")]
    MultipathIsNotEqual,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("already configured")]
    AlreadyConfigured,
    #[error("failed to load")]
    FailedToLoad,
    #[error("invalid argument")]
    InvalidArgument,
    #[error("invalid data")]
    InvalidData,
}
