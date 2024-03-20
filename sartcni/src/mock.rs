use std::{collections::HashMap, sync::Mutex};

use bytes::Bytes;

use tonic::{Request, Response, Status};

use crate::proto::{
    sart::{self, CniResult},
    CNIErrorDetail,
};

#[derive(Debug, Clone)]
pub(super) struct MockContainer {
    pub(super) cni_result: CniResult,
    pub(super) add: bool,
    pub(super) del: bool,
    pub(super) check: u32,
}

pub(super) struct MockCNIApiServer {
    containers: Mutex<HashMap<String, MockContainer>>,
}

impl MockCNIApiServer {
    pub(super) fn new(containers: HashMap<String, MockContainer>) -> MockCNIApiServer {
        MockCNIApiServer {
            containers: Mutex::new(containers),
        }
    }
}

#[tonic::async_trait]
impl sart::cni_api_server::CniApi for MockCNIApiServer {
    async fn add(&self, req: Request<sart::Args>) -> Result<Response<CniResult>, Status> {
        let container_id = req.get_ref().container_id.clone();
        let mut c = self.containers.lock().unwrap();
        let container = match c.get_mut(&container_id) {
            Some(container) => container,
            None => {
                let e = serde_json::to_vec(&CNIErrorDetail {
                    code: sart::ErrorCode::NotExist as u32,
                    msg: rscni::error::Error::NotExist(String::new()).to_string(),
                    details: format!("{container_id} is already added"),
                })
                .unwrap();
                let buf = Bytes::from(e);
                return Err(Status::with_details(
                    tonic::Code::NotFound,
                    "Container is not found",
                    buf,
                ));
            }
        };
        if container.add {
            let e = serde_json::to_vec(&CNIErrorDetail {
                code: sart::ErrorCode::AlreadyAdded as u32,
                msg: "Already Added".to_string(),
                details: format!("{container_id} is already added"),
            })
            .unwrap();

            let buf = Bytes::from(e);
            return Err(Status::with_details(
                tonic::Code::Aborted,
                "request aborted",
                buf,
            ));
        } else {
            container.add = true;
            return Ok(Response::new(container.cni_result.clone()));
        }
    }

    async fn del(&self, req: Request<sart::Args>) -> Result<Response<CniResult>, Status> {
        let container_id = req.get_ref().container_id.clone();
        let mut c = self.containers.lock().unwrap();
        let container = match c.get_mut(&container_id) {
            Some(container) => container,
            None => {
                let e = serde_json::to_vec(&CNIErrorDetail {
                    code: sart::ErrorCode::NotExist as u32,
                    msg: rscni::error::Error::NotExist(String::new()).to_string(),
                    details: format!("{container_id} is already added"),
                })
                .unwrap();
                let buf = Bytes::from(e);
                return Err(Status::with_details(
                    tonic::Code::NotFound,
                    "Container is not found",
                    buf,
                ));
            }
        };
        if container.del {
            Ok(Response::new(container.cni_result.clone()))
        } else {
            container.del = true;
            Ok(Response::new(container.cni_result.clone()))
        }
    }

    async fn check(&self, req: Request<sart::Args>) -> Result<Response<CniResult>, Status> {
        let container_id = req.get_ref().container_id.clone();
        let mut c = self.containers.lock().unwrap();
        let container = match c.get_mut(&container_id) {
            Some(container) => container,
            None => {
                let e = serde_json::to_vec(&CNIErrorDetail {
                    code: sart::ErrorCode::NotExist as u32,
                    msg: rscni::error::Error::NotExist(String::new()).to_string(),
                    details: format!("{container_id} is already added"),
                })
                .unwrap();
                let buf = Bytes::from(e);
                return Err(Status::with_details(
                    tonic::Code::NotFound,
                    "Container is not found",
                    buf,
                ));
            }
        };
        container.check += 1;
        Ok(Response::new(container.cni_result.clone()))
    }
}
