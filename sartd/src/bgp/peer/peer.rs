use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::future;
use tokio::net::TcpStream;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

use crate::bgp::{error::Error, packet::codec::Codec};

use super::{fsm::FiniteStateMachine, neighbor::Neighbor};

#[derive(Debug)]
pub(crate) struct Peer {
    neighbor: Neighbor,
    fsm: Mutex<FiniteStateMachine>,
}

impl Peer {
    pub async fn poll(&self) -> Result<(), Error> {
        loop {
            // let msg = self.codec.next().await
        }

        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct PeerManager {}

#[derive(Debug)]
pub(crate) struct PeerBuilder {
    remote_asn: u32,
    // remote_addr: SocketAddr,
    // local_addr: SocketAddr,
}

impl PeerBuilder {
    pub fn new(remote_asn: u32) -> Self {
        Self { remote_asn }
    }

    pub fn build(&self) -> Result<(), Error> {
        Ok(())
    }
}
