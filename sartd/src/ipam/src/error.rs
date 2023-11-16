use thiserror::Error;

use super::bitset::BitSetError;

#[derive(Debug, Error, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Error {
    #[error("BitSet error: {0}")]
    BitSet(#[source] BitSetError),

    #[error("Protocol mismatch")]
    ProtocolMistmatch,

    #[error("Not contains")]
    NotContains,

    #[error("No releasable address")]
    NoReleasableAddress,

    #[error("CIDR too large: {0}")]
    CIDRTooLarge(u8),

    #[error("Full")]
    Full,

    #[error("AddressBlock not found")]
    BlockNotFound,

    #[error("Auto assignable block already exists")]
    AutoAssignableBlockAlreadyExists,
}
