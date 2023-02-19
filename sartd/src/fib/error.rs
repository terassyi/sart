use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("failed to communicate with rtnetlink")]
    FailedToCommunicateWithNetlink(#[from] rtnetlink::Error),
}
