use std::net::IpAddr;

use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;

use crate::fib::rib::RequestType;
use crate::fib::server::api;
use crate::proto::sart::fib_api_server::{FibApi, FibApiServer};

use super::error::Error;
use super::route::NextHop;
use super::route::NextHopFlags;
use super::route::Route;

use tonic::{Request, Response, Status};

use crate::proto::sart::{
    AddMultiPathRouteRequest, AddRouteRequest, DeleteMultiPathRouteRequest, DeleteRouteRequest,
    GetRouteRequest, GetRouteResponse, ListRoutesRequest, ListRoutesResponse,
};

use super::route::ip_version_from;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct Bgp {
    pub endpoint: String,
}

impl Bgp {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }

    pub async fn subscribe(&self) -> Result<Receiver<(RequestType, Route)>, Error> {
        let (tx, rx) = tokio::sync::mpsc::channel::<(RequestType, Route)>(128);

        tracing::info!(endpoint = self.endpoint, "subscribe BGP at");
        let endpoint = self.endpoint.clone();
        tokio::spawn(async move {
            let sock_addr = endpoint.parse().unwrap();
            let reflection = tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(api::FILE_DESCRIPTOR_SET)
                .build()
                .unwrap();

            tonic::transport::Server::builder()
                .add_service(FibApiServer::new(BgpSubscriber::new(tx)))
                .add_service(reflection)
                .serve(sock_addr)
                .await
                .unwrap();
        });
        Ok(rx)
    }

    pub async fn publish(&self, route: Route) -> Result<(), Error> {
        Ok(())
    }
}

struct BgpSubscriber {
    queue: Sender<(RequestType, Route)>,
}

impl BgpSubscriber {
    pub fn new(queue: Sender<(RequestType, Route)>) -> Self {
        Self { queue }
    }
}

#[tonic::async_trait]
impl FibApi for BgpSubscriber {
    #[tracing::instrument(skip(self, req))]
    async fn get_route(
        &self,
        req: Request<GetRouteRequest>,
    ) -> Result<Response<GetRouteResponse>, Status> {
        Ok(Response::new(GetRouteResponse { route: None }))
    }

    #[tracing::instrument(skip(self, req))]
    async fn list_routes(
        &self,
        req: Request<ListRoutesRequest>,
    ) -> Result<Response<ListRoutesResponse>, Status> {
        Ok(Response::new(ListRoutesResponse { routes: vec![] }))
    }

    #[tracing::instrument(skip(self, req))]
    async fn add_route(&self, req: Request<AddRouteRequest>) -> Result<Response<()>, Status> {
        if let Some(route) = &req.get_ref().route {
            let route = match Route::try_from(route) {
                Ok(route) => route,
                Err(e) => return Err(Status::aborted(format!("{e}"))),
            };
            match self.queue.send((RequestType::AddRoute, route)).await {
                Ok(_) => Ok(Response::new(())),
                Err(e) => Err(Status::internal(format!("{e}"))),
            }
        } else {
            Err(Status::aborted("route is required"))
        }
    }

    #[tracing::instrument(skip(self, req))]
    async fn delete_route(&self, req: Request<DeleteRouteRequest>) -> Result<Response<()>, Status> {
        let ver = match ip_version_from(req.get_ref().version as u32) {
            Ok(ver) => ver,
            Err(_) => return Err(Status::aborted("invalid ip version")),
        };
        let dst: IpNet = match req.get_ref().destination.parse() {
            Ok(dst) => dst,
            Err(e) => return Err(Status::aborted("invalid destination prefix")),
        };
        let mut route = Route::default();
        route.destination = dst;
        route.version = ver;

        match self.queue.send((RequestType::DeleteRoute, route)).await {
            Ok(_) => Ok(Response::new(())),
            Err(e) => Err(Status::internal(format!("{e}"))),
        }
    }

    #[tracing::instrument(skip(self, req))]
    async fn add_multi_path_route(
        &self,
        req: Request<AddMultiPathRouteRequest>,
    ) -> Result<Response<()>, Status> {
        let ver = match ip_version_from(req.get_ref().version as u32) {
            Ok(ver) => ver,
            Err(_) => return Err(Status::aborted("invalid ip version")),
        };
        let dst: IpNet = match req.get_ref().destination.parse() {
            Ok(dst) => dst,
            Err(e) => return Err(Status::aborted("invalid destination prefix")),
        };

        let mut route = Route::default();
        route.destination = dst;
        route.version = ver;
        let next_hops = &req.get_ref().next_hops;

        for mut next_hop in next_hops.iter() {
            let mut nh = match NextHop::try_from(next_hop) {
                Ok(nh) => nh,
                Err(e) => return Err(Status::aborted(format!("{e}"))),
            };
            nh.weight = 1; // TODO: not to use fixed weight(=1)

            route.next_hops.push(nh);
        }

        match self
            .queue
            .send((RequestType::AddMultiPathRoute, route))
            .await
        {
            Ok(_) => Ok(Response::new(())),
            Err(e) => Err(Status::internal(format!("{e}"))),
        }
    }

    async fn delete_multi_path_route(
        &self,
        req: Request<DeleteMultiPathRouteRequest>,
    ) -> Result<Response<()>, Status> {
        let ver = match ip_version_from(req.get_ref().version as u32) {
            Ok(ver) => ver,
            Err(_) => return Err(Status::aborted("invalid ip version")),
        };
        let dst: IpNet = match req.get_ref().destination.parse() {
            Ok(dst) => dst,
            Err(e) => return Err(Status::aborted("invalid destination prefix")),
        };

        let mut route = Route::default();
        route.destination = dst;
        route.version = ver;
        let gateways = &req.get_ref().gateways;

        for gw in gateways.iter() {
            let addr: IpAddr = match gw.parse() {
                Ok(addr) => addr,
                Err(e) => return Err(Status::aborted(format!("{e}"))),
            };
            let nh = NextHop {
                gateway: addr,
                flags: NextHopFlags::Empty,
                weight: 1,
                interface: 0,
            };

            route.next_hops.push(nh);
        }

        match self
            .queue
            .send((RequestType::DeleteMultiPathRoute, route))
            .await
        {
            Ok(_) => Ok(Response::new(())),
            Err(e) => Err(Status::internal(format!("{e}"))),
        }
    }
}
