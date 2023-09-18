use async_trait::async_trait;
use std::{net::IpAddr, time::Duration};

use crate::{proto, error::Error, bgp::peer::Peer};

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
    pub fn new(endpoint: &str, timeout: u64) -> SartSpeaker {
        SartSpeaker {
            endpoint: endpoint.to_string(),
            timeout,
        }
    }
}


#[async_trait]
impl Speaker for SartSpeaker {
    async fn add_peer(&self, peer: Peer) -> Result<(), Error> {
        let mut client = tokio::time::timeout(Duration::from_secs(self.timeout), connect_bgp(&self.endpoint))
            .await
            .map_err(|_| Error::ClientTimeout)?;
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
}
