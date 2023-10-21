use kube::core::admission::SerializePatchError;
use thiserror::Error;

use crate::{kubernetes, trace::error::TraceableError};

#[derive(Error, Debug)]
pub(crate) enum Error {
    #[error("std::io::Error")]
    StdIoError(#[from] std::io::Error),

    #[error("config error")]
    ConfigError(#[from] ConfigError),

    #[error("SerializationError: {0}")]
    SerializationError(#[source] serde_json::Error),

    #[error("SerializePatchError: {0}")]
    SerializePatchError(#[source] SerializePatchError),

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

    #[error("CRD Error: {0}")]
    CRDError(#[source] kubernetes::crd::error::Error),

    #[error("Label matching Error: {0}")]
    LabelMatchingError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

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
