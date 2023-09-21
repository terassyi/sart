use async_trait::async_trait;
use std::{net::IpAddr, time::Duration};

use crate::{proto, error::Error, bgp::{peer::{Peer, DEFAULT_HOLD_TIME, DEFAULT_KEEPALIVE_TIME}, common::Protocol}};

use super::speaker::Speaker;

pub(crate) const CLIENT_TIMEOUT: u64 = 10;

pub(crate) async fn connect_bgp(
    endpoint: &str,
) -> proto::sart::bgp_api_client::BgpApiClient<tonic::transport::Channel> {
    let endpoint_url = format!("http://{}", endpoint);
    proto::sart::bgp_api_client::BgpApiClient::connect(endpoint_url)
        .await
        .unwrap()
}

#[allow(dead_code)]
pub(crate) async fn connect_fib(endpoint: &str) -> proto::sart::fib_manager_api_client::FibManagerApiClient<tonic::transport::Channel> {
    let endpoint_url = format!("http://{}", endpoint);
    proto::sart::fib_manager_api_client::FibManagerApiClient::connect(endpoint_url)
        .await
        .unwrap()
}

pub(crate) struct SartSpeaker {
    pub endpoint: String,
    timeout: u64,
}

impl SartSpeaker {
}


#[async_trait]
impl Speaker for SartSpeaker {
    fn new(endpoint: &str, timeout: u64) -> Self {
        SartSpeaker {
            endpoint: endpoint.to_string(),
            timeout,
        }
    }
    async fn add_peer(&self, peer: Peer) -> Result<(), Error> {
        let mut client = tokio::time::timeout(Duration::from_secs(self.timeout), connect_bgp(&self.endpoint))
            .await
            .map_err(|_| Error::ClientTimeout)?;

        client.add_peer(proto::sart::AddPeerRequest{
            peer: Some(proto::sart::Peer{
                asn: peer.neighbor.asn,
                address: peer.neighbor.addr.clone(),
                router_id: peer.neighbor.addr.clone(),
                families: vec![
                    protocol_to_address_family(peer.protocol),
                ],
                hold_time: match peer.hold_time {
                    Some(t) => t as u32,
                    None => DEFAULT_HOLD_TIME as u32,
                },
                keepalive_time: match peer.keepalive_time {
                    Some(t) => t as u32,
                    None => DEFAULT_KEEPALIVE_TIME as u32,
                },
                uptime: None,
                send_counter: None,
                recv_counter: None,
                state: 1,
                passive_open: false,
            })
        })
        .await
        .map_err(|status| Error::GRPCError(status))?;
        Ok(())
    }

    async fn delete_peer(&self, addr: IpAddr) -> Result<(), Error> {
        let mut client = tokio::time::timeout(Duration::from_secs(self.timeout), connect_bgp(&self.endpoint))
            .await
            .map_err(|_| Error::ClientTimeout)?;

        client.delete_peer(proto::sart::DeletePeerRequest{
            addr: addr.to_string(),
        })
        .await
        .map_err(|status| Error::GRPCError(status))?;
        
        Ok(())
    }

    async fn get_peer(&self, addr: IpAddr) -> Result<Peer, Error> {
        let mut client = tokio::time::timeout(Duration::from_secs(self.timeout), connect_bgp(&self.endpoint))
            .await
            .map_err(|_| Error::ClientTimeout)?;

        let res = client.get_neighbor(proto::sart::GetNeighborRequest{
            addr: addr.to_string(),
        })
        .await
        .map_err(|status| Error::GRPCError(status))?
        .get_ref()
        .peer
        .clone()
        .ok_or(Error::FailedToGetData("Peer information from speaker".to_string()))?;

        Ok(Peer::try_from(&res)?)
    }
}

fn protocol_to_address_family(protocol: Protocol) -> proto::sart::AddressFamily {
    proto::sart::AddressFamily {
        afi: match protocol {
            Protocol::Unknown => proto::sart::address_family::Afi::Unknown as i32,
            Protocol::IPv4 => proto::sart::address_family::Afi::Ip4 as i32,
            Protocol::IPv6 => proto::sart::address_family::Afi::Ip6 as i32,
        },
        safi: proto::sart::address_family::Safi::Unicast as i32,
    }
}
