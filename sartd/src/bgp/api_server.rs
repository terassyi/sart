use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

use crate::proto::sart::bgp_api_server::{BgpApi, BgpApiServer};
use crate::proto::sart::*;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Notify;
use tonic::{transport::Server, Request, Response, Status};
use tonic_reflection::server::Builder;

use super::event::ControlEvent;

pub mod api {
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("bgp");
}

#[derive(Debug, Clone)]
pub(crate) struct ApiServer {
    tx: Sender<ControlEvent>,
    signal: Arc<Notify>,
}

impl ApiServer {
    pub fn new(tx: Sender<ControlEvent>, signal: Arc<Notify>) -> Self {
        Self { tx, signal }
    }
}

#[tonic::async_trait]
impl BgpApi for ApiServer {
    async fn health(&self, req: Request<HealthRequest>) -> Result<Response<()>, Status> {
        self.tx.send(ControlEvent::Health).await.unwrap();
        self.signal.notified().await;
        Ok(Response::new(()))
    }
}
