use std::{
    collections::HashMap,
    net::Ipv4Addr,
    str::FromStr,
    sync::{Arc, Mutex},
};

use sartd_proto::sart::{
    bgp_api_server::{BgpApi, BgpApiServer},
    AddPathRequest, AddPathResponse, AddPeerRequest, BgpInfo, ClearBgpInfoRequest,
    DeletePathRequest, DeletePathResponse, DeletePeerRequest, GetBgpInfoRequest,
    GetBgpInfoResponse, GetNeighborPathRequest, GetNeighborPathResponse, GetNeighborRequest,
    GetNeighborResponse, GetPathByPrefixRequest, GetPathByPrefixResponse, GetPathRequest,
    GetPathResponse, HealthRequest, ListNeighborRequest, ListNeighborResponse, Path, Peer,
    SetAsRequest, SetRouterIdRequest, ConfigureMultiPathRequest,
};
use tonic::{transport::Server, Request, Response, Status};

#[derive(Clone, Debug, Default)]
pub struct MockBgpApiServer {
    inner: Arc<Mutex<MockBgpApiServerInner>>,
}

#[derive(Clone, Debug, Default)]
pub struct MockBgpApiServerInner {
    pub asn: Option<u32>,
    pub router_id: Option<Ipv4Addr>,
    pub peers: HashMap<Ipv4Addr, Peer>,
    pub paths: Vec<Path>,
    pub multi_path: bool,
}

impl MockBgpApiServer {
    fn new_with(inner: Arc<Mutex<MockBgpApiServerInner>>) -> Self {
        Self { inner }
    }
}

impl MockBgpApiServerInner {
    pub fn new_with(asn: u32, router_id: &str) -> Self {
        Self {
            asn: Some(asn),
            router_id: Some(Ipv4Addr::from_str(router_id).unwrap()),
            peers: HashMap::new(),
            paths: Vec::new(),
            multi_path: false,
        }
    }
}

pub async fn run(port: u32) {
    let sock_addr = format!("0.0.0.0:{port}").parse().unwrap();

    Server::builder()
        .add_service(BgpApiServer::new(MockBgpApiServer::default()))
        .serve(sock_addr)
        .await
        .unwrap();
}

pub async fn run_with(inner: Arc<Mutex<MockBgpApiServerInner>>, port: u32) {
    let sock_addr = format!("0.0.0.0:{port}").parse().unwrap();

    Server::builder()
        .add_service(BgpApiServer::new(MockBgpApiServer::new_with(inner)))
        .serve(sock_addr)
        .await
        .unwrap();
}

#[tonic::async_trait]
impl BgpApi for MockBgpApiServer {
    async fn health(&self, _req: Request<HealthRequest>) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }

    async fn get_bgp_info(
        &self,
        _req: Request<GetBgpInfoRequest>,
    ) -> Result<Response<GetBgpInfoResponse>, Status> {
        let (asn, router_id, multi_path) = {
            let inner = self.inner.lock().unwrap();
            (
                inner.asn.unwrap_or(0),
                inner
                    .router_id
                    .map(|r| r.to_string())
                    .unwrap_or("".to_string()),
                inner.multi_path,
            )
        };
        Ok(Response::new(GetBgpInfoResponse {
            info: Some(BgpInfo {
                asn,
                router_id,
                port: 179,
                multi_path,
            }),
        }))
    }

    async fn get_neighbor(
        &self,
        req: Request<GetNeighborRequest>,
    ) -> Result<Response<GetNeighborResponse>, Status> {
        let id = match Ipv4Addr::from_str(&req.get_ref().addr) {
            Ok(id) => id,
            Err(e) => return Err(Status::aborted(e.to_string())),
        };
        let peer = {
            let inner = self.inner.lock().unwrap();
            inner.peers.get(&id).cloned()
        };
        if peer.is_none() {
            return Err(Status::not_found("peer not found"));
        }
        Ok(Response::new(GetNeighborResponse { peer }))
    }

    async fn list_neighbor(
        &self,
        _req: Request<ListNeighborRequest>,
    ) -> Result<Response<ListNeighborResponse>, Status> {
        let mut peers = Vec::new();
        {
            let inner = self.inner.lock().unwrap();
            for (_id, peer) in inner.peers.iter() {
                peers.push(peer.clone())
            }
        }

        Ok(Response::new(ListNeighborResponse { peers }))
    }

    async fn get_path(
        &self,
        req: Request<GetPathRequest>,
    ) -> Result<Response<GetPathResponse>, Status> {
        let req_family = req.get_ref().family.as_ref().unwrap().clone();
        let mut paths = Vec::new();
        {
            let inner = self.inner.lock().unwrap();
            for path in inner.paths.iter() {
                let family = path.family.as_ref().unwrap();
                if family.eq(&req_family) {
                    paths.push(path.clone());
                }
            }
        }
        Ok(Response::new(GetPathResponse { paths }))
    }

    async fn get_neighbor_path(
        &self,
        _req: Request<GetNeighborPathRequest>,
    ) -> Result<Response<GetNeighborPathResponse>, Status> {
        Ok(Response::new(GetNeighborPathResponse { paths: vec![] }))
    }

    async fn get_path_by_prefix(
        &self,
        _req: Request<GetPathByPrefixRequest>,
    ) -> Result<Response<GetPathByPrefixResponse>, Status> {
        Ok(Response::new(GetPathByPrefixResponse { paths: vec![] }))
    }

    async fn set_as(&self, req: Request<SetAsRequest>) -> Result<Response<()>, Status> {
        let asn = req.get_ref().asn;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.asn = Some(asn);
        }
        Ok(Response::new(()))
    }

    async fn set_router_id(
        &self,
        req: Request<SetRouterIdRequest>,
    ) -> Result<Response<()>, Status> {
        let router_id = match Ipv4Addr::from_str(&req.get_ref().router_id) {
            Ok(r) => r,
            Err(e) => return Err(Status::aborted(e.to_string())),
        };
        {
            let mut inner = self.inner.lock().unwrap();
            inner.router_id = Some(router_id);
        }
        Ok(Response::new(()))
    }

    async fn configure_multi_path(&self, req: Request<ConfigureMultiPathRequest>) -> Result<Response<()>, Status> {
        let enable = req.get_ref().enable;
        {
            let mut inner = self.inner.lock().unwrap();
            inner.multi_path = enable;
        }

        Ok(Response::new(()))
    }

    async fn clear_bgp_info(
        &self,
        _req: Request<ClearBgpInfoRequest>,
    ) -> Result<Response<()>, Status> {
        {
            let mut inner = self.inner.lock().unwrap();
            inner.asn = None;
            inner.router_id = None;
        }
        Ok(Response::new(()))
    }

    async fn add_peer(&self, req: Request<AddPeerRequest>) -> Result<Response<()>, Status> {
        let peer = match &req.get_ref().peer {
            Some(p) => p.clone(),
            None => return Err(Status::aborted("request must have peer")),
        };
        let id = match Ipv4Addr::from_str(&peer.router_id) {
            Ok(id) => id,
            Err(e) => return Err(Status::aborted(e.to_string())),
        };
        {
            let mut inner = self.inner.lock().unwrap();
            inner.peers.insert(id, peer);
        }
        Ok(Response::new(()))
    }

    async fn delete_peer(&self, req: Request<DeletePeerRequest>) -> Result<Response<()>, Status> {
        let id = match Ipv4Addr::from_str(&req.get_ref().addr) {
            Ok(id) => id,
            Err(e) => return Err(Status::aborted(e.to_string())),
        };
        {
            let mut inner = self.inner.lock().unwrap();
            inner.peers.remove(&id);
        }
        Ok(Response::new(()))
    }

    async fn add_path(
        &self,
        req: Request<AddPathRequest>,
    ) -> Result<Response<AddPathResponse>, Status> {
        let family = req.get_ref().family.as_ref().unwrap().clone();
        let prefixs = req.get_ref().prefixes.clone();
        let paths = prefixs.iter().map(|p| Path {
            family: Some(family.clone()),
            nlri: p.clone(),
            ..Default::default()
        });
        {
            let mut inner = self.inner.lock().unwrap();
            for p in paths.into_iter() {
                inner.paths.push(p);
            }
        }
        Ok(Response::new(AddPathResponse {}))
    }

    async fn delete_path(
        &self,
        req: Request<DeletePathRequest>,
    ) -> Result<Response<DeletePathResponse>, Status> {
        let family = req.get_ref().family.as_ref().unwrap().clone();
        let prefixs = req.get_ref().prefixes.clone();
        {
            let mut inner = self.inner.lock().unwrap();
            let mut new_paths = Vec::new();
            for p in prefixs.iter() {
                for path in inner.paths.iter() {
                    if path.family.as_ref().unwrap().ne(&family) || path.nlri.ne(p) {
                        new_paths.push(path.clone());
                    }
                }
            }
            inner.paths = new_paths;
        }

        Ok(Response::new(DeletePathResponse {}))
    }
}
