use std::net::{IpAddr, Ipv4Addr};

use crate::proto::sart::bgp_api_server::{BgpApi, BgpApiServer};
use crate::proto::sart::*;
use tonic::{transport::Server, Request, Response, Status};
use tonic_reflection::server::Builder;

mod api {
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("bgp");
}

#[derive(Debug, Copy, Clone)]
pub(crate) struct ApiServer {
    addr: IpAddr,
    port: u16,
}

impl ApiServer {
    pub fn new(port: u16) -> Self {
        Self {
            addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            port,
        }
    }

    pub async fn serve(&self) -> Result<(), Box<dyn std::error::Error>> {
        let sock_addr = format!("{}:{}", self.addr, self.port).parse().unwrap();

        let reflection_server = Builder::configure()
            .register_encoded_file_descriptor_set(api::FILE_DESCRIPTOR_SET)
            .build()?;

        Server::builder()
            .add_service(BgpApiServer::new(*self))
            .add_service(reflection_server)
            .serve(sock_addr)
            .await?;
        Ok(())
    }
}

#[tonic::async_trait]
impl BgpApi for ApiServer {
    async fn health(&self, req: Request<HealthRequest>) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }
}
