use std::time::{Duration, Instant};

use crate::agent::error::Error;



pub(crate) async fn connect_bgp(
    endpoint: &str,
) -> Result<crate::proto::sart::bgp_api_client::BgpApiClient<tonic::transport::Channel>, Error> {
    let endpoint_url = format!("http://{}", endpoint);
    crate::proto::sart::bgp_api_client::BgpApiClient::connect(endpoint_url)
        .await
        .map_err(Error::FailedToCommunicateWithgRPC)
}

#[tracing::instrument]
pub(crate) async fn connect_bgp_with_retry(endpoint: &str, timeout: Duration) -> Result<crate::proto::sart::bgp_api_client::BgpApiClient<tonic::transport::Channel>, Error> {
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() > deadline {
            return Err(Error::Timeout)
        }
        match connect_bgp(endpoint).await {
            Ok(conn) => return Ok(conn),
            Err(e) => {
                tracing::error!(error=?e,"failed to connect bgp");
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}
