use std::marker::{Send, Sync};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use ipnet::IpNet;
use sartd_proto::sart::bgp_api_server::BgpApi;
use sartd_proto::sart::{
    AddPathRequest, AddPathResponse, AddPeerRequest, BgpInfo, ClearBgpInfoRequest,
    ConfigureMultiPathRequest, DeletePathRequest, DeletePathResponse, DeletePeerRequest,
    GetBgpInfoRequest, GetBgpInfoResponse, GetNeighborPathRequest, GetNeighborPathResponse,
    GetNeighborRequest, GetNeighborResponse, GetPathByPrefixRequest, GetPathByPrefixResponse,
    GetPathRequest, GetPathResponse, HealthRequest, ListNeighborRequest, ListNeighborResponse,
    Path, Peer, SetAsRequest, SetRouterIdRequest,
};
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{Mutex, Notify};
use tokio::time::timeout;
use tonic::{Request, Response, Status};

use super::config::NeighborConfig;
use super::rib::RibKind;
use super::{event::ControlEvent, family::AddressFamily, packet::attribute::Attribute};

#[derive(Debug)]
pub struct ApiServer {
    tx: Mutex<Sender<ControlEvent>>,
    response_rx: Mutex<Receiver<ApiResponse>>,
    timeout: u64,
    internal_timeout: u64,
    signal: Arc<Notify>,
}

impl ApiServer {
    pub fn new(
        tx: Sender<ControlEvent>,
        response_rx: Receiver<ApiResponse>,
        timeout: u64,
        internal_timeout: u64,
        signal: Arc<Notify>,
    ) -> Self {
        Self {
            tx: Mutex::new(tx),
            response_rx: Mutex::new(response_rx),
            timeout,
            internal_timeout,
            signal,
        }
    }
}

#[tonic::async_trait]
impl BgpApi for ApiServer {
    async fn health(&self, _req: Request<HealthRequest>) -> Result<Response<()>, Status> {
        let guard_tx = self.tx.lock().await;
        guard_tx.send(ControlEvent::Health).await.unwrap();
        self.signal.notified().await;
        Ok(Response::new(()))
    }

    async fn get_bgp_info(
        &self,
        _req: Request<GetBgpInfoRequest>,
    ) -> Result<Response<GetBgpInfoResponse>, Status> {
        let guard_tx = self.tx.lock().await;
        guard_tx.send(ControlEvent::GetBgpInfo).await.unwrap();
        let mut rx = self.response_rx.lock().await;
        match timeout(Duration::from_secs(self.internal_timeout), rx.recv()).await {
            Ok(res) => match res {
                Some(info) => {
                    if let ApiResponse::BgpInfo(info) = info {
                        Ok(Response::new(GetBgpInfoResponse { info: Some(info) }))
                    } else {
                        Err(Status::internal("failed to get bgp information"))
                    }
                }
                None => Err(Status::internal("failed to get bgp information")),
            },
            Err(_e) => Err(Status::deadline_exceeded("timeout")),
        }
    }

    #[tracing::instrument(skip_all)]
    async fn get_neighbor(
        &self,
        req: Request<GetNeighborRequest>,
    ) -> Result<Response<GetNeighborResponse>, Status> {
        let addr = match req.get_ref().addr.parse() {
            Ok(addr) => addr,
            Err(_) => return Err(Status::aborted("failed to parse peer address")),
        };
        let guard_tx = self.tx.lock().await;
        guard_tx.send(ControlEvent::GetPeer(addr)).await.unwrap();

        let mut rx = self.response_rx.lock().await;
        match timeout(Duration::from_secs(self.internal_timeout), rx.recv()).await {
            Ok(res) => match res {
                Some(res) => {
                    if let ApiResponse::Neighbor(peer) = res {
                        Ok(Response::new(sartd_proto::sart::GetNeighborResponse {
                            peer: Some(peer),
                        }))
                    } else {
                        tracing::error!(result=?res,"failed to get a neighbor information");
                        return Err(Status::internal("failed to get neighbor information"));
                    }
                }
                None => {
                    tracing::error!(result = "None", "failed to get neighbor information");
                    return Err(Status::internal("failed to get neighbor information"));
                }
            },
            Err(_e) => Err(Status::not_found("peer not found")),
        }
    }

    async fn list_neighbor(
        &self,
        _req: Request<ListNeighborRequest>,
    ) -> Result<Response<ListNeighborResponse>, Status> {
        let guard_tx = self.tx.lock().await;
        guard_tx.send(ControlEvent::ListPeer).await.unwrap();
        let mut rx = self.response_rx.lock().await;
        match timeout(Duration::from_secs(self.internal_timeout), rx.recv()).await {
            Ok(res) => match res {
                Some(res) => {
                    if let ApiResponse::Neighbors(peers) = res {
                        Ok(Response::new(ListNeighborResponse { peers }))
                    } else {
                        Err(Status::internal("failed to get neighbors list information"))
                    }
                }
                None => Err(Status::internal("failed to get neighbors list information")),
            },
            Err(_e) => Err(Status::not_found("peers not found")),
        }
    }

    async fn get_path(
        &self,
        req: Request<GetPathRequest>,
    ) -> Result<Response<GetPathResponse>, Status> {
        if req.get_ref().family.is_none() {
            return Err(Status::aborted("failed to receive AddressFamily"));
        }
        let f = req.get_ref().family.as_ref().unwrap();
        let family = match AddressFamily::new(f.afi as u16, f.safi as u8) {
            Ok(f) => f,
            Err(_) => return Err(Status::aborted("failed to get AddressFamily")),
        };
        let guard_tx = self.tx.lock().await;
        guard_tx.send(ControlEvent::GetPath(family)).await.unwrap();

        let mut rx = self.response_rx.lock().await;
        match timeout(Duration::from_secs(self.internal_timeout), rx.recv()).await {
            Ok(res) => match res {
                Some(res) => {
                    if let ApiResponse::Paths(paths) = res {
                        Ok(Response::new(GetPathResponse { paths }))
                    } else {
                        Err(Status::internal("failed to get path information"))
                    }
                }
                None => Err(Status::internal("failed to get path information")),
            },
            Err(_e) => Err(Status::not_found("path not found")),
        }
    }

    async fn get_neighbor_path(
        &self,
        req: Request<GetNeighborPathRequest>,
    ) -> Result<Response<GetNeighborPathResponse>, Status> {
        if req.get_ref().family.is_none() {
            return Err(Status::aborted("failed to receive AddressFamily"));
        }
        let f = req.get_ref().family.as_ref().unwrap();
        let family = match AddressFamily::new(f.afi as u16, f.safi as u8) {
            Ok(f) => f,
            Err(_) => return Err(Status::aborted("failed to get AddressFamily")),
        };
        let addr = match req.get_ref().addr.parse() {
            Ok(addr) => addr,
            Err(_) => return Err(Status::aborted("failed to parse peer address")),
        };
        let kind = match RibKind::try_from(req.get_ref().kind as u32) {
            Ok(k) => k,
            Err(_) => return Err(Status::aborted("failed to get rib kind")),
        };

        let guard_tx = self.tx.lock().await;
        guard_tx
            .send(ControlEvent::GetNeighborPath(kind, addr, family))
            .await
            .unwrap();

        let mut rx = self.response_rx.lock().await;
        match timeout(Duration::from_secs(self.internal_timeout), rx.recv()).await {
            Ok(res) => match res {
                Some(res) => {
                    if let ApiResponse::Paths(paths) = res {
                        Ok(Response::new(GetNeighborPathResponse { paths }))
                    } else {
                        Err(Status::internal("failed to get path information"))
                    }
                }
                None => Err(Status::internal("failed to get path information")),
            },
            Err(_e) => Err(Status::not_found("neighbor's paths not found")),
        }
    }

    async fn get_path_by_prefix(
        &self,
        req: Request<GetPathByPrefixRequest>,
    ) -> Result<Response<GetPathByPrefixResponse>, Status> {
        let prefix = match IpNet::from_str(&req.get_ref().prefix) {
            Ok(p) => p,
            Err(e) => return Err(Status::aborted(format!("failed to parse prefix: {}", e))),
        };
        if req.get_ref().family.is_none() {
            return Err(Status::aborted("failed to receive AddressFamily"));
        }
        let f = req.get_ref().family.as_ref().unwrap();
        let family = match AddressFamily::new(f.afi as u16, f.safi as u8) {
            Ok(f) => f,
            Err(_) => return Err(Status::aborted("failed to get AddressFamily")),
        };
        let guard_tx = self.tx.lock().await;
        guard_tx
            .send(ControlEvent::GetPathByPrefix(prefix, family))
            .await
            .unwrap();

        let mut rx = self.response_rx.lock().await;
        match timeout(Duration::from_secs(self.internal_timeout), rx.recv()).await {
            Ok(res) => match res {
                Some(res) => {
                    if let ApiResponse::Paths(paths) = res {
                        Ok(Response::new(GetPathByPrefixResponse { paths }))
                    } else {
                        Err(Status::internal("failed to get path information"))
                    }
                }
                None => Err(Status::internal("failed to get path information")),
            },
            Err(_e) => Err(Status::not_found("path not found")),
        }
    }

    async fn set_as(&self, req: Request<SetAsRequest>) -> Result<Response<()>, Status> {
        let guard_tx = self.tx.lock().await;
        match guard_tx.send(ControlEvent::SetAsn(req.get_ref().asn)).await {
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
        let guard_tx = self.tx.lock().await;
        match guard_tx.send(ControlEvent::SetRouterId(router_id)).await {
            Ok(_) => Ok(Response::new(())),
            Err(_) => Err(Status::aborted("failed to send rib event")),
        }
    }

    async fn configure_multi_path(
        &self,
        req: Request<ConfigureMultiPathRequest>,
    ) -> Result<Response<()>, Status> {
        let enable = req.get_ref().enable;
        let guard_tx = self.tx.lock().await;
        match guard_tx
            .send(ControlEvent::ConfigureMultiPath(enable))
            .await
        {
            Ok(_) => Ok(Response::new(())),
            Err(_) => Err(Status::aborted("failed to send rib event")),
        }
    }

    async fn clear_bgp_info(
        &self,
        _req: Request<ClearBgpInfoRequest>,
    ) -> Result<Response<()>, Status> {
        let guard_tx = self.tx.lock().await;
        match guard_tx.send(ControlEvent::ClearBgpInfo).await {
            Ok(_) => Ok(Response::new(())),
            Err(_) => Err(Status::internal("failed to clear BGP server information")),
        }
    }

    #[tracing::instrument(skip(self, req))]
    async fn add_peer(&self, req: Request<AddPeerRequest>) -> Result<Response<()>, Status> {
        let neighbor_config = match &req.get_ref().peer {
            Some(peer) => match NeighborConfig::try_from(peer) {
                Ok(config) => config,
                Err(e) => return Err(Status::aborted(e.to_string())),
            },
            None => return Err(Status::aborted("Neighbor information is not set")),
        };
        let guard_tx = self.tx.lock().await;
        match guard_tx.send(ControlEvent::AddPeer(neighbor_config)).await {
            Ok(_) => Ok(Response::new(())),
            Err(_) => Err(Status::aborted("failed to send rib event")),
        }
    }

    async fn delete_peer(&self, req: Request<DeletePeerRequest>) -> Result<Response<()>, Status> {
        let addr = match req.get_ref().addr.parse() {
            Ok(addr) => addr,
            Err(_) => return Err(Status::aborted("failed to parse peer addr as IpAddr")),
        };
        let guard_tx = self.tx.lock().await;
        match guard_tx.send(ControlEvent::DeletePeer(addr)).await {
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
        let guard_tx = self.tx.lock().await;
        match guard_tx
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
        let guard_tx = self.tx.lock().await;
        match guard_tx
            .send(ControlEvent::DeletePath(family, prefixes))
            .await
        {
            Ok(_) => Ok(Response::new(DeletePathResponse {})),
            Err(_) => Err(Status::aborted("failed to send rib event")),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ApiResponse {
    BgpInfo(BgpInfo),
    Neighbor(Peer),
    Neighbors(Vec<Peer>),
    Path(Path),
    Paths(Vec<Path>),
}

unsafe impl Send for ApiResponse {}
unsafe impl Sync for ApiResponse {}
