use thiserror::Error;

use super::bitset::BitSetError;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("BitSet error: {0}")]
    BitSet(#[source] BitSetError),
}
