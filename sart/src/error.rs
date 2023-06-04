use thiserror::Error;
use tonic::Status;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("missing argument: {}", msg)]
    MissingArgument { msg: String },
    #[error("invalid RPC response")]
    InvalidRPCResponse,
    #[error("failed to get Response")]
    FailedToGetResponse(#[from] Status),
    #[error("unacceptable attribute")]
    UnacceptableAttribute,
    #[error("invalid origin value: acceptable")]
    InvalidOriginValue,
    #[error("invalid channel type")]
    InvalidChannelType,
}
