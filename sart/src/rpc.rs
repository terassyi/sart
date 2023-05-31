use crate::proto;

pub(crate) async fn connect_bgp(
    endpoint: &str,
) -> proto::sart::bgp_api_client::BgpApiClient<tonic::transport::Channel> {
    let endpoint_url = format!("http://{}", endpoint);
    proto::sart::bgp_api_client::BgpApiClient::connect(endpoint_url)
        .await
        .unwrap()
}

pub(crate) async fn connect_fib(endpoint: &str) -> proto::sart::fib_manager_api_client::FibManagerApiClient<tonic::transport::Channel> {
    let endpoint_url = format!("http://{}", endpoint);
    proto::sart::fib_manager_api_client::FibManagerApiClient::connect(endpoint_url)
        .await
        .unwrap()
}
