use thiserror::Error;

use crate::{kubernetes, trace::error::TraceableError};

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("std::io::Error")]
    StdIoError(#[from] std::io::Error),

    #[error("Kube Error: {0}")]
    KubeError(#[source] kube::Error),

    #[error("config error")]
    ConfigError(#[from] ConfigError),

    #[error("failed to communicate with rtnetlink: {}", e)]
    FailedToCommunicateWithNetlink {
        #[from]
        e: rtnetlink::Error,
    },
    #[error("failed to communicate with gRPC server/client")]
    FailedToCommunicateWithgRPC(#[from] tonic::transport::Error),
    #[error("timeout")]
    Timeout,
    #[error("got error {0} from gRPC")]
    GotgPRCError(#[from] tonic::Status),

    #[error("Local BGP speaker is not configured")]
    LocalSpeakerIsNotConfigured,

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("FailedToGetData: {0}")]
    FailedToGetData(String),

    #[error("SerializationError: {0}")]
    SerializationError(#[source] serde_json::Error),

    #[error("CRD Error: {0}")]
    CRDError(#[source] kubernetes::crd::error::Error),
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

impl TraceableError for Error {
    fn metric_label(&self) -> String {
        format!("{self:?}").to_lowercase()
    }
}
