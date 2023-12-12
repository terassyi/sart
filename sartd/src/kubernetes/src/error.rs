use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Get Namespace Error")]
    GetNamespace,
}
