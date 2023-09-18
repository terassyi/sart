use async_trait::async_trait;
use std::net::IpAddr;

use crate::{error::Error, bgp::peer::Peer};

#[async_trait]
pub(crate) trait Speaker {
	async fn add_peer(&self, peer: Peer) -> Result<(), Error>;
	async fn delete_peer(&self, addr: IpAddr) -> Result<(), Error>;
}
