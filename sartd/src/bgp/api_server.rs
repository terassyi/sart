use std::{net::Ipv4Addr, sync::Arc};

use crate::proto::sart::bgp_api_server::BgpApi;
use crate::proto::sart::*;
use ipnet::IpNet;
use tokio::sync::mpsc::Sender;
use tokio::sync::Notify;
use tonic::{Request, Response, Status};

use super::{
    event::{ControlEvent, RibEvent},
    family::AddressFamily,
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

    #[tracing::instrument(skip(self, req))]
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
        match self
            .tx
            .send(ControlEvent::AddPath(prefixes, attributes))
            .await
        {
            Ok(_) => Ok(Response::new(AddPathResponse {})),
            Err(_) => Err(Status::aborted("failed to send rib event")),
        }
    }

    #[tracing::instrument(skip(self, req))]
    async fn delete_path(
        &self,
        req: Request<DeletePathRequest>,
    ) -> Result<Response<DeletePathResponse>, Status> {
        let family = match &req.get_ref().family {
            Some(family) => AddressFamily::new(family.afi as u16, family.safi as u8).unwrap(),
            None => {
                let n: IpNet = req.get_ref().prefixes.first().unwrap().parse().unwrap(); // a request must have at lease one prefix
                match n {
                    IpNet::V4(_) => AddressFamily::ipv4_unicast(),
                    IpNet::V6(_) => AddressFamily::ipv6_unicast(),
                }
            }
        };
        let mut prefixes = Vec::new();
        for p in req.get_ref().prefixes.iter() {
            match p.parse() {
                Ok(p) => prefixes.push(p),
                Err(e) => return Err(Status::aborted(format!("invalid network format: {:?}", e))),
            }
        }
        match self
            .tx
            .send(ControlEvent::DeletePath(family, prefixes))
            .await
        {
            Ok(_) => Ok(Response::new(DeletePathResponse {})),
            Err(_) => Err(Status::aborted("failed to send rib event")),
        }
    }
}
