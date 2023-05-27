use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;
use tokio::sync::mpsc::Sender;

use crate::{
    fib::api_server::api,
    proto::sart::fib_api_server::{FibApi, FibApiServer},
};

use super::error::Error;

use tonic::{Request, Response, Status};

use crate::proto::sart::{
    AddMultiPathRouteRequest, AddRouteRequest, DeleteMultiPathRouteRequest, DeleteRouteRequest,
    GetRouteRequest, GetRouteResponse, ListRoutesRequest, ListRoutesResponse,
};

use super::rib::Route;
use super::route::{ip_version_from, RtClient};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Bgp {
    pub endpoint: String,
}

impl Bgp {
    pub fn new(endpoint: String) -> Self {
        Self { endpoint }
    }

    pub async fn subscribe(&self) -> Result<(Receiver<Route>), Error> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Route>(128);

        let sock_addr = self.endpoint.parse().unwrap();
        let reflection = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(api::FILE_DESCRIPTOR_SET)
            .build()
            .unwrap();

        tracing::info!("start to listen at {}", self.endpoint);
        tonic::transport::Server::builder()
            .add_service(FibApiServer::new(BgpSubscriber::new(tx)))
            .add_service(reflection)
            .serve(sock_addr)
            .await
            .unwrap();
        Ok(rx)
    }

    pub fn publish(&self) -> Result<(), Error> {
        Ok(())
    }
}

struct BgpSubscriber {
    queue: Sender<Route>,
}

impl BgpSubscriber {
    pub fn new(queue: Sender<Route>) -> Self {
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
        let table_id = req.get_ref().table;
        let ver = match ip_version_from(req.get_ref().version as u32) {
            Ok(ver) => ver,
            Err(_) => return Err(Status::aborted("invalid ip version")),
        };
        let dst = if req.get_ref().destination == "default" {
            "0.0.0.0"
        } else {
            &req.get_ref().destination
        };
        Ok(Response::new(GetRouteResponse { route: None }))
    }

    #[tracing::instrument(skip(self, req))]
    async fn list_routes(
        &self,
        req: Request<ListRoutesRequest>,
    ) -> Result<Response<ListRoutesResponse>, Status> {
        let table_id = req.get_ref().table;
        let ver = match ip_version_from(req.get_ref().version as u32) {
            Ok(ver) => ver,
            Err(_) => return Err(Status::aborted("invalid ip version")),
        };
        Ok(Response::new(ListRoutesResponse { routes: vec![] }))
    }

    #[tracing::instrument(skip(self, req))]
    async fn add_route(&self, req: Request<AddRouteRequest>) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }

    #[tracing::instrument(skip(self, req))]
    async fn delete_route(&self, req: Request<DeleteRouteRequest>) -> Result<Response<()>, Status> {
        let table_id = req.get_ref().table as u8;
        let ver = match ip_version_from(req.get_ref().version as u32) {
            Ok(ver) => ver,
            Err(_) => return Err(Status::aborted("invalid ip version")),
        };
        Ok(Response::new(()))
    }

    #[tracing::instrument(skip(self, req))]
    async fn add_multi_path_route(
        &self,
        req: Request<AddMultiPathRouteRequest>,
    ) -> Result<Response<()>, Status> {
        let table = req.get_ref().table as u8;
        let ver = match ip_version_from(req.get_ref().version as u32) {
            Ok(ver) => ver,
            Err(_) => return Err(Status::aborted("invalid ip version")),
        };
        Ok(Response::new(()))
    }

    async fn delete_multi_path_route(
        &self,
        req: Request<DeleteMultiPathRouteRequest>,
    ) -> Result<Response<()>, Status> {
        let table = req.get_ref().table as u8;
        let ver = match ip_version_from(req.get_ref().version as u32) {
            Ok(ver) => ver,
            Err(_) => return Err(Status::aborted("invalid ip version")),
        };

        Ok(Response::new(()))
    }
}
