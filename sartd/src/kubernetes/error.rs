use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("Get Namespace Error")]
    GetNamespace,
}
