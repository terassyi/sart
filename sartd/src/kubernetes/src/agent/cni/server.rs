use std::sync::Arc;

use kube::Client;
use sartd_ipam::manager::AllocatorSet;
use sartd_proto::sart::{cni_api_server::CniApi, Args, CniResult};
use tonic::{async_trait, Request, Response, Status};

pub(crate) struct CNIServer {
    client: Client,
    allocator_set: Arc<AllocatorSet>,
}

#[async_trait]
impl CniApi for CNIServer {
    async fn add(&self, req: Request<Args>) -> Result<Response<CniResult>, Status> {
        Err(Status::aborted(""))
    }
    async fn del(&self, req: Request<Args>) -> Result<Response<CniResult>, Status> {
        Err(Status::aborted(""))
    }
    async fn check(&self, req: Request<Args>) -> Result<Response<CniResult>, Status> {
        Err(Status::aborted(""))
    }
}
