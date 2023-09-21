use async_trait::async_trait;
use std::net::IpAddr;

use crate::{error::Error, bgp::peer::Peer};

pub(crate) const DEFAULT_ENDPOINT_CONNECT_TIMEOUT: u64 = 10;

#[async_trait]
pub(crate) trait Speaker {
	fn new(endpoint: &str, timeout: u64) -> Self;
	async fn add_peer(&self, peer: Peer) -> Result<(), Error>;
	async fn delete_peer(&self, addr: IpAddr) -> Result<(), Error>;
	async fn get_peer(&self, addr: IpAddr) -> Result<Peer, Error>;
}

pub(crate) fn new<T: Speaker>(endpoint: &str, timeout: u64) -> T {
	T::new(endpoint, timeout)
}
