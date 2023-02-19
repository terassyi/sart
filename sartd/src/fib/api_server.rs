use futures::TryStreamExt;
use tonic::{Request, Response, Status};

use crate::{
    fib::error::Error,
    proto::sart::{fib_api_server::FibApi, GetPathResponse, GetRoutesRequest, GetRoutesResponse},
};

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
    async fn get_routes(
        &self,
        _req: Request<GetRoutesRequest>,
    ) -> Result<Response<GetRoutesResponse>, Status> {
        self.rt.get_routes().await.unwrap();
        Ok(Response::new(GetRoutesResponse {}))
    }
}

#[derive(Debug)]
struct RtClient {
    handler: rtnetlink::Handle,
}

impl RtClient {
    fn new(handler: rtnetlink::Handle) -> RtClient {
        RtClient { handler }
    }

    async fn get_routes(&self) -> Result<(), Error> {
        let route = self.handler.route();
        let mut res = route.get(rtnetlink::IpVersion::V4).execute();
        while let Some(route) = res.try_next().await? {
            println!("{route:?}");
        }
        Ok(())
    }
}
