use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("std::io::Error")]
    StdIoError(#[from] std::io::Error),

    #[error("Kube Error: {0}")]
    KubeError(#[source] kube::Error),

    #[error("Validation Error: {0}")]
    ValidationError(String),

    #[error("Invalid Peer State: {0}")]
    InvalidPeerState(i32),
}
