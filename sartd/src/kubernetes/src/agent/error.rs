use sartd_trace::error::TraceableError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("std::io::Error")]
    StdIo(#[from] std::io::Error),

    #[error("Failed to get lock")]
    FailedToGetLock,

    #[error("Var Error: {0}")]
    Var(#[source] std::env::VarError),

    #[error("Kube Error: {0}")]
    Kube(#[source] kube::Error),

    #[error("config error")]
    Config(#[from] ConfigError),

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
    GotgPRC(#[from] tonic::Status),

    #[error("Local BGP speaker is not configured")]
    LocalSpeakerIsNotConfigured,

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    Finalizer(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("FailedToGetData: {0}")]
    FailedToGetData(String),

    #[error("SerializationError: {0}")]
    Serialization(#[source] serde_json::Error),

    #[error("CRD Error: {0}")]
    CRD(#[source] crate::crd::error::Error),

    #[error("Kubernetes Library Error: {0}")]
    KubeLibrary(#[source] crate::error::Error),

    #[error("Ipam Error: {0}")]
    Ipam(#[source] sartd_ipam::error::Error),

    #[error("Peer exists")]
    PeerExists,

    #[error("Invalid CIDR")]
    InvalidCIDR,

    #[error("Auto assignable pool already exists")]
    AutoAssignAlreadyExists,

    #[error("Cannot delete")]
    CannotDelete,

    #[error("Not empty")]
    NotEmpty,

    #[error("Failed to notify")]
    FailedToNotify,

    #[error("Missing fields: {0}")]
    MissingFields(String),
}

#[derive(Debug, Error, Clone)]
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
