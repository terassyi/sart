use tonic::{Request, Response, Status};

use crate::proto::sart::{fib_api_server::FibApi, GetRoutesRequest, GetRoutesResponse};

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
    async fn get_routes(
        &self,
        req: Request<GetRoutesRequest>,
    ) -> Result<Response<GetRoutesResponse>, Status> {
        let table_id = req.get_ref().table;
        let ver = match ip_version_from(req.get_ref().version as u32) {
            Ok(ver) => ver,
            Err(_) => return Err(Status::aborted("invalid ip version")),
        };
        match self.rt.get_routes(table_id, ver).await {
            Ok(routes) => Ok(Response::new(GetRoutesResponse { routes })),
            Err(e) => Err(Status::internal(format!("{}", e))),
        }
    }
}
