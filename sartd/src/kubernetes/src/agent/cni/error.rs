use thiserror::Error;

use super::{netlink, netns};

#[derive(Debug, Error)]
pub enum Error {
    #[error("NetNS: {0}")]
    NetNS(#[source] netns::Error),

    #[error("Netlink: {0}")]
    Netlink(#[source] netlink::Error),

    #[error("Kubernetes: {0}")]
    Kube(#[source] kube::Error),

    #[error("Missing fields: {0}")]
    MissingField(String),

    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    #[error("Failed to get lock")]
    Lock,

    #[error("Pod already configured")]
    AlreadyConfigured(String),

    #[error("Default pool not found")]
    DefaultPoolNotFound,

    #[error("Block not found")]
    BlockNotFound(String),

    #[error("Failed to receive notification")]
    ReceiveNotify,

    #[error("Ipam: {0}")]
    Ipam(#[source] sartd_ipam::error::Error),

    #[error("Pod address is not found")]
    PodAddressIsNotFound,

    #[error("Allocation not found")]
    AllocationNotFound,

    #[error("Addresses don't match")]
    AddressNotMatched,
}
