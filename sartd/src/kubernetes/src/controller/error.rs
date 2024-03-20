use kube::core::admission::SerializePatchError;
use sartd_trace::error::TraceableError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("std::io::Error")]
    StdIo(#[from] std::io::Error),

    #[error("failed to get lock")]
    FailedToGetLock,

    #[error("config error")]
    Config(#[from] ConfigError),

    #[error("SerializationError: {0}")]
    Serialization(#[source] serde_json::Error),

    #[error("SerializePatchError: {0}")]
    SerializePatch(#[source] SerializePatchError),

    #[error("Kube Error: {0}")]
    Kube(#[source] kube::Error),

    #[error("gRPC Error: {0}")]
    GRPC(tonic::Status),

    #[error("gRPC Connection Error: {0}")]
    GRPCConnection(#[source] tonic::transport::Error),

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    Finalizer(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("CRD Error: {0}")]
    CRD(#[source] crate::crd::error::Error),

    #[error("Kube Library Error: {0}")]
    KubeLibrary(#[source] crate::error::Error),

    #[error("Ipam Error: {0}")]
    Ipam(#[source] sartd_ipam::error::Error),

    #[error("Label matching Error: {0}")]
    LabelMatching(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Node not found")]
    NodeNotFound,

    #[error("Peer already exists")]
    PeerAlreadyExists,

    #[error("Address not found")]
    AddressNotFound,

    #[error("Invalid Address")]
    InvalidAddress,

    #[error("ASN not found")]
    AsnNotFound,

    #[error("Invalid ASN value")]
    InvalidAsnValue,

    #[error("Client timeout")]
    ClientTimeout,

    #[error("Invalid protocol")]
    InvalidProtocol,

    #[error("FailedToGetData: {0}")]
    FailedToGetData(String),

    #[error("Invalid endpoint")]
    InvalidEndpoint,

    #[error("Invalid ExternalTrafficPolicy")]
    InvalidExternalTrafficPolicy,

    #[error("allocatable address is not found")]
    NoAllocatableAddress,

    #[error("no available address block")]
    NoAllocatableBlock,

    #[error("Failed to release address")]
    ReleaseAddress,

    #[error("Unavailable allocation")]
    UnavailableAllocation,

    #[error("Withdrawing yet")]
    Withdrawing,

    #[error("Auto assignable pool must be one")]
    AutoAssignMustBeOne,

    #[error("Failed to enable auto assign")]
    FailedToEnableAutoAssign,

    #[error("Block is not empty")]
    NotEmpty,

    #[error("Cannot delete")]
    CannotDelete,

    #[error("BlockRequest already exists")]
    BlockRequestAlreadyExists,

    #[error("BlockRequest not performed")]
    BlockRequestNotPerformed,

    #[error("Invalid address type")]
    InvalidAddressType,

    #[error("Invalid CIDR")]
    InvalidCIDR,

    #[error("Invalid pool")]
    InvalidPool,
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
