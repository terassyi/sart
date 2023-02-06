use std::{net::Ipv4Addr, sync::Arc};

use crate::proto::sart::bgp_api_server::BgpApi;
use crate::proto::sart::*;
use tokio::sync::mpsc::Sender;
use tokio::sync::Notify;
use tonic::{Request, Response, Status};

use super::{
    event::{ControlEvent, RibEvent},
    packet::attribute::{self, Attribute},
};

pub mod api {
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("bgp");
}

#[derive(Debug, Clone)]
pub(crate) struct ApiServer {
    asn: u32,
    router_id: Ipv4Addr,
    tx: Sender<ControlEvent>,
    signal: Arc<Notify>,
}

impl ApiServer {
    pub fn new(
        asn: u32,
        router_id: Ipv4Addr,
        tx: Sender<ControlEvent>,
        signal: Arc<Notify>,
    ) -> Self {
        Self {
            asn,
            router_id,
            tx,
            signal,
        }
    }
}

#[tonic::async_trait]
impl BgpApi for ApiServer {
    async fn health(&self, _req: Request<HealthRequest>) -> Result<Response<()>, Status> {
        self.tx.send(ControlEvent::Health).await.unwrap();
        self.signal.notified().await;
        Ok(Response::new(()))
    }

    async fn add_path(
        &self,
        req: Request<AddPathRequest>,
    ) -> Result<Response<AddPathResponse>, Status> {
        let mut prefixes = Vec::new();
        for p in req.get_ref().prefixes.iter() {
            match p.parse() {
                Ok(p) => prefixes.push(p),
                Err(e) => return Err(Status::aborted(format!("invalid network format: {:?}", e))),
            }
        }
        let mut attributes = Vec::new();
        for attr in req.get_ref().attributes.iter() {
            match Attribute::try_from(attr.clone()) {
                Ok(attr) => attributes.push(attr),
                Err(e) => {
                    return Err(Status::aborted(format!(
                        "invalid attribute format: {:?}",
                        e
                    )))
                }
            }
        }
        let event = RibEvent::AddPath(prefixes, attributes);
        Ok(Response::new(AddPathResponse {}))
    }

    async fn delete_path(
        &self,
        req: Request<DeletePathRequest>,
    ) -> Result<Response<DeletePathResponse>, Status> {
        Ok(Response::new(DeletePathResponse {}))
    }
}
