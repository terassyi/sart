use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum Error {
    #[error("SerializationError: {0}")]
    SerializationError(#[source] serde_json::Error),

    #[error("Kube Error: {0}")]
    KubeError(#[source] kube::Error),

    #[error("gRPC Error: {0}")]
    GRPCError(tonic::Status),

    #[error("gRPC Connection Error: {0}")]
    GRPCConnectionError(#[source] tonic::transport::Error),

    #[error("Finalizer Error: {0}")]
    // NB: awkward type because finalizer::Error embeds the reconciler error (which is this)
    // so boxing this error to break cycles
    FinalizerError(#[source] Box<kube::runtime::finalizer::Error<Error>>),

    #[error("Label matching Error: {0}")]
    LabelMatchingError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Address not found")]
    AddressNotFound,

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
}
pub(crate) type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn metric_label(&self) -> String {
        format!("{self:?}").to_lowercase()
    }
}
