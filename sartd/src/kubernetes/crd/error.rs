use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("std::io::Error")]
    StdIo(#[from] std::io::Error),

    #[error("Kube Error: {0}")]
    Kube(#[source] kube::Error),

    #[error("Validation Error: {0}")]
    Validation(String),

    #[error("Invalid Peer State: {0}")]
    InvalidPeerState(i32),
}
