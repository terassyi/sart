use tonic::{Request, Response, Status};

use crate::proto::sart::{
    fib_api_server::FibApi, AddMultiPathRouteRequest, AddRouteRequest, DeleteMultiPathRouteRequest,
    DeleteRouteRequest, GetRouteRequest, GetRouteResponse, ListRoutesRequest, ListRoutesResponse,
};

use super::route::{ip_version_from, RtClient};

pub mod api {
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("sartd");
}

#[derive(Debug)]
pub(crate) struct FibServer {
    rt: RtClient,
}

impl FibServer {
    pub fn new(handler: rtnetlink::Handle) -> FibServer {
        FibServer {
            rt: RtClient::new(handler),
        }
    }
}

#[tonic::async_trait]
impl FibApi for FibServer {
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
        match self.rt.get_route(table_id as u8, ver, dst).await {
            Ok(route) => Ok(Response::new(GetRouteResponse { route: Some(route) })),
            Err(e) => Err(Status::internal(format!("{}", e))),
        }
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
        match self.rt.list_routes(table_id as u8, ver).await {
            Ok(routes) => Ok(Response::new(ListRoutesResponse { routes })),
            Err(e) => Err(Status::internal(format!("{}", e))),
        }
    }

    #[tracing::instrument(skip(self, req))]
    async fn add_route(&self, req: Request<AddRouteRequest>) -> Result<Response<()>, Status> {
        if let Some(route) = &req.get_ref().route {
            match self.rt.add_route(route, req.get_ref().replace).await {
                Ok(_) => Ok(Response::new(())),
                Err(e) => Err(Status::internal(format!("{e}"))),
            }
        } else {
            Err(Status::aborted("route is required"))
        }
    }

    #[tracing::instrument(skip(self, req))]
    async fn delete_route(&self, req: Request<DeleteRouteRequest>) -> Result<Response<()>, Status> {
        let table_id = req.get_ref().table as u8;
        let ver = match ip_version_from(req.get_ref().version as u32) {
            Ok(ver) => ver,
            Err(_) => return Err(Status::aborted("invalid ip version")),
        };
        let dest = match req.get_ref().destination.parse() {
            Ok(dest) => dest,
            Err(_) => return Err(Status::aborted("failed to parse destination")),
        };

        match self.rt.delete_route(table_id, ver, dest).await {
            Ok(_) => Ok(Response::new(())),
            Err(e) => Err(Status::internal(format!("{}", e))),
        }
    }

    #[tracing::instrument(skip(self, req))]
    async fn add_multi_path_route(
        &self,
        req: Request<AddMultiPathRouteRequest>,
    ) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }

    async fn delete_multi_path_route(
        &self,
        req: Request<DeleteMultiPathRouteRequest>,
    ) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }
}
