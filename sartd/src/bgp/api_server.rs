use std::sync::Arc;

use crate::proto::sart::bgp_api_server::BgpApi;
use crate::proto::sart::*;
use ipnet::IpNet;
use tokio::sync::mpsc::Sender;
use tokio::sync::Notify;
use tonic::{Request, Response, Status};

use super::config::NeighborConfig;
use super::{event::ControlEvent, family::AddressFamily, packet::attribute::Attribute};

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
    async fn health(&self, _req: Request<HealthRequest>) -> Result<Response<()>, Status> {
        self.tx.send(ControlEvent::Health).await.unwrap();
        self.signal.notified().await;
        Ok(Response::new(()))
    }

    async fn show(
        &self,
        req: Request<BgpShowRequest>,
    ) -> Result<Response<BgpShowResponse>, Status> {
        Err(Status::aborted("message"))
    }

    async fn set_as(&self, req: Request<SetAsRequest>) -> Result<Response<()>, Status> {
        match self.tx.send(ControlEvent::SetAsn(req.get_ref().asn)).await {
            Ok(_) => Ok(Response::new(())),
            Err(_) => Err(Status::aborted("failed to send rib event")),
        }
    }

    async fn set_router_id(
        &self,
        req: Request<SetRouterIdRequest>,
    ) -> Result<Response<()>, Status> {
        let router_id = match req.get_ref().router_id.parse() {
            Ok(id) => id,
            Err(_) => return Err(Status::aborted("failed to parse router_id as Ipv4Addr")),
        };
        match self.tx.send(ControlEvent::SetRouterId(router_id)).await {
            Ok(_) => Ok(Response::new(())),
            Err(_) => Err(Status::aborted("failed to send rib event")),
        }
    }

    #[tracing::instrument(skip(self, req))]
    async fn add_peer(&self, req: Request<AddPeerRequest>) -> Result<Response<()>, Status> {
        let neighbor_config = match &req.get_ref().peer {
            Some(peer) => match NeighborConfig::try_from(peer) {
                Ok(config) => config,
                Err(_) => return Err(Status::aborted("")),
            },
            None => return Err(Status::aborted("")),
        };
        match self.tx.send(ControlEvent::AddPeer(neighbor_config)).await {
            Ok(_) => Ok(Response::new(())),
            Err(_) => Err(Status::aborted("failed to send rib event")),
        }
    }

    async fn delete_peer(&self, req: Request<DeletePeerRequest>) -> Result<Response<()>, Status> {
        let addr = match req.get_ref().addr.parse() {
            Ok(addr) => addr,
            Err(_) => return Err(Status::aborted("failed to parse peer addr as IpAddr")),
        };
        match self.tx.send(ControlEvent::DeletePeer(addr)).await {
            Ok(_) => Ok(Response::new(())),
            Err(_) => Err(Status::aborted("failed to send rib event")),
        }
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

#[derive(Debug)]
pub(crate) struct ApiResponse {}
