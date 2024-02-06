use std::{collections::HashMap, net::IpAddr, path::Path, str::FromStr, sync::Arc};

use bytes::Bytes;
use ipnet::IpNet;
use k8s_openapi::api::core::v1::{Namespace, Pod};
use kube::{
    api::{DeleteParams, ListParams, PostParams},
    core::ObjectMeta,
    Api, Client, ResourceExt,
};
use rscni::error::Error;
use sartd_ipam::manager::AllocatorSet;
use sartd_proto::sart::{
    cni_api_server::{CniApi, CniApiServer},
    Args, CniResult, Interface, IpConf, RouteConf,
};
use serde::{Deserialize, Serialize};
use tokio::{
    net::UnixListener,
    sync::{mpsc::UnboundedReceiver, Mutex},
};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{async_trait, transport::Server, Request, Response, Status};

use crate::crd::{
    address_block::AddressBlock,
    address_pool::{AddressPool, AddressType, ADDRESS_POOL_ANNOTATION},
    block_request::{BlockRequest, BlockRequestSpec},
};

use super::pod::{PodAllocation, PodInfo};

pub const CNI_SERVER_ENDPOINT: &str = "/var/run/sart.sock";

const CNI_ERROR_CODE_KUBE: u32 = 200;
const CNI_ERROR_MSG_KUBE: &str = "Kubernetes error";
const CNI_ERROR_CODE_INTERNAL: u32 = 210;
const CNI_ERROR_MSG_INTERNAL: &str = "Internal error";
const CNI_ERROR_CODE_ALLOCATOR: u32 = 220;
const CNI_ERROR_MSG_ALLOCATOR: &str = "Allocation error";

struct CNIServerInner {
    client: Client,
    allocator: Arc<AllocatorSet>,
    allocation: HashMap<String, PodAllocation>,
    node: String,
    receiver: UnboundedReceiver<AddressBlock>,
}

pub(crate) struct CNIServer {
    // consider some better type.
    inner: Arc<Mutex<CNIServerInner>>,
}

impl CNIServer {
    pub fn new(
        client: Client,
        allocator: Arc<AllocatorSet>,
        node: String,
        receiver: UnboundedReceiver<AddressBlock>,
    ) -> CNIServer {
        CNIServer {
            inner: Arc::new(Mutex::new(CNIServerInner::new(
                client, allocator, node, receiver,
            ))),
        }
    }
}

impl CNIServerInner {
    pub fn new(
        client: Client,
        allocator: Arc<AllocatorSet>,
        node: String,
        receiver: UnboundedReceiver<AddressBlock>,
    ) -> CNIServerInner {
        CNIServerInner {
            client,
            allocator,
            allocation: HashMap::new(),
            node,
            receiver,
        }
    }

    #[tracing::instrument(skip_all)]
    async fn add(&mut self, args: &Args) -> Result<CniResult, Error> {
        let pod_info = PodInfo::from_str(&args.args)?;

        tracing::info!(
            name = pod_info.name,
            namespace = pod_info.namespace,
            container_id = pod_info.container_id,
            uid = pod_info.uid,
            cmd = "ADD",
            "CNI Add is called"
        );

        let pod_api = Api::<Pod>::namespaced(self.client.clone(), &pod_info.namespace);
        let pod = pod_api.get(&pod_info.name).await.map_err(|e| {
            Error::Custom(
                CNI_ERROR_CODE_KUBE,
                CNI_ERROR_MSG_KUBE.to_string(),
                e.to_string(),
            )
        })?;
        let pool = self.get_pool(&pod_info, &pod).await?;

        let allocated_addr = self.allocate(&pool)?;

        let alloc = match allocated_addr {
            Some(alloc) => {
                tracing::info!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    uid = pod_info.uid,
                    address = alloc.addr.to_string(),
                    block = alloc.block,
                    cmd = "ADD",
                    "Allocate addresses"
                );
                alloc
            }
            None => {
                // request
                tracing::warn!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    uid = pod_info.uid,
                    cmd = "ADD",
                    "No allocatable block. Request new block."
                );
                let block_request_api = Api::<BlockRequest>::all(self.client.clone());
                let br = BlockRequest {
                    metadata: ObjectMeta {
                        name: Some(format!("{pool}-{}", self.node)),
                        ..Default::default()
                    },
                    spec: BlockRequestSpec {
                        pool,
                        node: self.node.clone(),
                    },
                    status: None,
                };
                block_request_api
                    .create(&PostParams::default(), &br)
                    .await
                    .map_err(|e| {
                        Error::Custom(
                            CNI_ERROR_CODE_KUBE,
                            CNI_ERROR_MSG_KUBE.to_string(),
                            e.to_string(),
                        )
                    })?;

                tracing::info!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    uid = pod_info.uid,
                    cmd = "ADD",
                    "Waiting to create new block"
                );
                // let created_br = self.receiver.blocking_recv();
                let created_br = self.receiver.recv().await.ok_or(Error::Custom(
                    CNI_ERROR_CODE_INTERNAL,
                    CNI_ERROR_MSG_INTERNAL.to_string(),
                    "Failed to notify block creation".to_string(),
                ))?;
                tracing::info!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    uid = pod_info.uid,
                    cmd = "ADD",
                    "Delete the satisfied block request"
                );
                block_request_api
                    .delete(&br.name_any(), &DeleteParams::default())
                    .await
                    .map_err(|e| {
                        Error::Custom(
                            CNI_ERROR_CODE_KUBE,
                            CNI_ERROR_MSG_KUBE.to_string(),
                            e.to_string(),
                        )
                    })?;
                let alloc = self.allocate_with_block(&created_br.name_any())?;
                tracing::info!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    uid = pod_info.uid,
                    address = alloc.addr.to_string(),
                    block = alloc.block,
                    cmd = "ADD",
                    "Allocate addresses"
                );
                alloc
            }
        };

        self.allocation.insert(
            format!("{}/{}", pod_info.namespace, pod_info.name),
            alloc.clone(),
        );

        let result = CniResult {
            interfaces: vec![Interface {
                name: "dummy".to_string(),
                mac: "dummy".to_string(),
                sandbox: "/var/run/dummy".to_string(),
            }],
            ips: vec![IpConf {
                interface: 0,
                address: alloc.addr.to_string(),
                gateway: "10.1.0.100".to_string(),
            }],
            routes: vec![RouteConf {
                dst: "0.0.0.0/0".to_string(),
                gw: "10.1.0.100".to_string(),
                mtu: 0,
                advmss: 0,
            }],
            dns: None,
        };

        Ok(result)
    }

    async fn del(&mut self, args: &Args) -> Result<CniResult, Error> {
        let pod_info = PodInfo::from_str(&args.args)?;

        tracing::info!(
            name = pod_info.name,
            namespace = pod_info.namespace,
            container_id = pod_info.container_id,
            uid = pod_info.uid,
            cmd = "DEL",
            "CNI Del is called"
        );

        let pod_api = Api::<Pod>::namespaced(self.client.clone(), &pod_info.namespace);
        let pod = pod_api.get(&pod_info.name).await.map_err(|e| {
            Error::Custom(
                CNI_ERROR_CODE_KUBE,
                CNI_ERROR_MSG_KUBE.to_string(),
                e.to_string(),
            )
        })?;
        let addr_opt = get_pod_addr(&pod);

        let _pool = self.get_pool(&pod_info, &pod).await?;
        if addr_opt.is_none() {
            tracing::warn!(
                name = pod_info.name,
                namespace = pod_info.namespace,
                container_id = pod_info.container_id,
                uid = pod_info.uid,
                cmd = "DEL",
                "Pod's address doesn't exist"
            );
        }
        let alloc_opt = self.get_allocation(&pod_info);
        if alloc_opt.is_none() {
            tracing::warn!(
                name = pod_info.name,
                namespace = pod_info.namespace,
                container_id = pod_info.container_id,
                uid = pod_info.uid,
                cmd = "DEL",
                "Allocation information doesn't exist"
            );
        }

        // get actual assigned address in pod netns

        let result = match alloc_opt {
            Some(alloc) => match self.release(&alloc.block, &alloc.addr.addr()) {
                Ok(_) => {
                    tracing::info!(
                        name = pod_info.name,
                        namespace = pod_info.namespace,
                        container_id = pod_info.container_id,
                        uid = pod_info.uid,
                        address = alloc.addr.to_string(),
                        block = alloc.block,
                        cmd = "DEL",
                        "Release allocated address"
                    );
                    self.allocation
                        .remove(&format!("{}/{}", pod_info.namespace, pod_info.name));
                    CniResult {
                        interfaces: Vec::new(),
                        ips: Vec::new(),
                        routes: Vec::new(),
                        dns: None,
                    }
                }
                Err(e) => {
                    tracing::error!(
                        name = pod_info.name,
                        namespace = pod_info.namespace,
                        container_id = pod_info.container_id,
                        uid = pod_info.uid,
                        cmd = "DEL",
                        error=?e,
                        "Failed to release address allocation",
                    );
                    CniResult {
                        interfaces: Vec::new(),
                        ips: Vec::new(),
                        routes: Vec::new(),
                        dns: None,
                    }
                }
            },
            None => CniResult {
                interfaces: Vec::new(),
                ips: Vec::new(),
                routes: Vec::new(),
                dns: None,
            },
        };

        Ok(result)
    }

    #[tracing::instrument(skip_all)]
    async fn check(&self, args: &Args) -> Result<CniResult, Error> {
        let pod_info = PodInfo::from_str(&args.args)?;

        tracing::info!(
            name = pod_info.name,
            namespace = pod_info.namespace,
            container_id = pod_info.container_id,
            uid = pod_info.uid,
            cmd = "CHECK",
            "CNI Check is called"
        );

        let pod_api = Api::<Pod>::namespaced(self.client.clone(), &pod_info.namespace);
        let pod = pod_api.get(&pod_info.name).await.map_err(|e| {
            Error::Custom(
                CNI_ERROR_CODE_KUBE,
                CNI_ERROR_MSG_KUBE.to_string(),
                e.to_string(),
            )
        })?;
        let pool = self.get_pool(&pod_info, &pod).await?;
        let pod_addr = get_pod_addr(&pod).ok_or(Error::Custom(
            CNI_ERROR_CODE_ALLOCATOR,
            CNI_ERROR_MSG_ALLOCATOR.to_string(),
            "Pod doesn't have an address".to_string(),
        ))?;
        let alloc = self.get_allocation(&pod_info).ok_or(Error::Custom(
            CNI_ERROR_CODE_ALLOCATOR,
            CNI_ERROR_MSG_ALLOCATOR.to_string(),
            "Allocation information not found".to_string(),
        ))?;
        if alloc.addr.addr().ne(&pod_addr) {
            return Err(Error::Custom(
                CNI_ERROR_CODE_ALLOCATOR,
                CNI_ERROR_MSG_ALLOCATOR.to_string(),
                "Pod address and actual allocation is mismatched".to_string(),
            ));
        }

        if !self.is_allocated(&pool, &alloc.addr.addr()) {
            return Err(Error::Custom(
                CNI_ERROR_CODE_ALLOCATOR,
                CNI_ERROR_MSG_ALLOCATOR.to_string(),
                "Allocator result and actual allocation is mismatched".to_string(),
            ));
        }

        Ok(CniResult {
            interfaces: Vec::new(),
            ips: Vec::new(),
            routes: Vec::new(),
            dns: None,
        })
    }

    async fn get_pool(&self, pod_info: &PodInfo, pod: &Pod) -> Result<String, Error> {
        let namespace_api = Api::<Namespace>::all(self.client.clone());
        let address_pool_api = Api::<AddressPool>::all(self.client.clone());

        let pool_opt = match pod.annotations().get(ADDRESS_POOL_ANNOTATION) {
            Some(pool) => Some(pool.to_string()),
            None => {
                let ns = namespace_api.get(&pod_info.namespace).await.map_err(|e| {
                    Error::Custom(
                        CNI_ERROR_CODE_KUBE,
                        CNI_ERROR_MSG_KUBE.to_string(),
                        e.to_string(),
                    )
                })?;
                ns.annotations().get(ADDRESS_POOL_ANNOTATION).cloned()
            }
        };
        let pool = match pool_opt {
            Some(pool) => pool,
            None => {
                let default = {
                    let allocator = self.allocator.inner.lock().map_err(|e| {
                        Error::Custom(
                            CNI_ERROR_CODE_INTERNAL,
                            CNI_ERROR_MSG_INTERNAL.to_string(),
                            e.to_string(),
                        )
                    })?;
                    allocator.auto_assign.clone()
                };
                match default {
                    Some(pool) => pool,
                    None => {
                        // In case of completely first pod allocation,
                        let ap_list = address_pool_api
                            .list(&ListParams::default())
                            .await
                            .map_err(|e| {
                                Error::Custom(
                                    CNI_ERROR_CODE_KUBE,
                                    CNI_ERROR_MSG_KUBE.to_string(),
                                    e.to_string(),
                                )
                            })?;
                        match ap_list.into_iter().find(|p| {
                            p.spec.auto_assign.unwrap_or(false)
                                && p.spec.r#type.eq(&AddressType::Pod)
                        }) {
                            Some(default_pool) => default_pool.name_any(),
                            None => {
                                return Err(Error::Custom(
                                    CNI_ERROR_CODE_INTERNAL,
                                    CNI_ERROR_MSG_INTERNAL.to_string(),
                                    "Default pool is not found".to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        };

        Ok(pool)
    }

    fn get_allocation(&self, pod_info: &PodInfo) -> Option<PodAllocation> {
        self.allocation
            .get(&format!("{}/{}", pod_info.namespace, pod_info.name))
            .cloned()
    }

    fn allocate(&mut self, pool: &str) -> Result<Option<PodAllocation>, Error> {
        let mut allocator = self.allocator.inner.lock().map_err(|e| {
            Error::Custom(
                CNI_ERROR_CODE_INTERNAL,
                CNI_ERROR_MSG_INTERNAL.to_string(),
                e.to_string(),
            )
        })?;
        let mut block_names: Vec<String> = allocator
            .blocks
            .values()
            .filter(|b| b.pool_name.eq(pool))
            .map(|b| b.name.clone())
            .collect();
        block_names.sort();
        for b in block_names.iter() {
            if let Some(block) = allocator.blocks.get_mut(b) {
                if block.allocator.is_full() {
                    continue;
                }
                // allocate!
                let prefix = block.allocator.prefix_len();
                let addr = block.allocator.allocate_next().map_err(|e| {
                    Error::Custom(
                        CNI_ERROR_CODE_ALLOCATOR,
                        CNI_ERROR_MSG_ALLOCATOR.to_string(),
                        e.to_string(),
                    )
                })?;
                let cidr = IpNet::new(addr, prefix).map_err(|e| {
                    Error::Custom(
                        CNI_ERROR_CODE_ALLOCATOR,
                        CNI_ERROR_MSG_ALLOCATOR.to_string(),
                        e.to_string(),
                    )
                })?;
                return Ok(Some(PodAllocation {
                    block: block.name.clone(),
                    addr: cidr,
                }));
            }
        }
        Ok(None)
    }

    fn allocate_with_block(&mut self, name: &str) -> Result<PodAllocation, Error> {
        let mut allocator = self.allocator.inner.lock().map_err(|e| {
            Error::Custom(
                CNI_ERROR_CODE_INTERNAL,
                CNI_ERROR_MSG_INTERNAL.to_string(),
                e.to_string(),
            )
        })?;
        let block = allocator.blocks.get_mut(name).ok_or(Error::Custom(
            CNI_ERROR_CODE_ALLOCATOR,
            CNI_ERROR_MSG_ALLOCATOR.to_string(),
            "Failed to get the internal allocator block".to_string(),
        ))?;
        let prefix = block.allocator.prefix_len();
        let addr = block.allocator.allocate_next().map_err(|e| {
            Error::Custom(
                CNI_ERROR_CODE_ALLOCATOR,
                CNI_ERROR_MSG_ALLOCATOR.to_string(),
                e.to_string(),
            )
        })?;
        let cidr = IpNet::new(addr, prefix).map_err(|e| {
            Error::Custom(
                CNI_ERROR_CODE_ALLOCATOR,
                CNI_ERROR_MSG_ALLOCATOR.to_string(),
                e.to_string(),
            )
        })?;
        Ok(PodAllocation {
            block: name.to_string(),
            addr: cidr,
        })
    }

    fn release(&mut self, pool: &str, addr: &IpAddr) -> Result<(), Error> {
        let mut allocator = self.allocator.inner.lock().map_err(|e| {
            Error::Custom(
                CNI_ERROR_CODE_INTERNAL,
                CNI_ERROR_MSG_INTERNAL.to_string(),
                e.to_string(),
            )
        })?;
        let block = allocator.blocks.get_mut(pool).ok_or(Error::Custom(
            CNI_ERROR_CODE_ALLOCATOR,
            CNI_ERROR_MSG_ALLOCATOR.to_string(),
            "Failed to get the internal allocator block".to_string(),
        ))?;
        block.allocator.release(addr).map_err(|e| {
            Error::Custom(
                CNI_ERROR_CODE_ALLOCATOR,
                CNI_ERROR_MSG_ALLOCATOR.to_string(),
                e.to_string(),
            )
        })?;

        Ok(())
    }

    fn is_allocated(&self, pool: &str, addr: &IpAddr) -> bool {
        let allocator = match self.allocator.inner.lock() {
            Ok(allocator) => allocator,
            Err(_) => return false,
        };
        let block = match allocator.blocks.get(pool) {
            Some(block) => block,
            None => return false,
        };
        block.allocator.is_allocated(addr)
    }
}

fn get_pod_addr(pod: &Pod) -> Option<IpAddr> {
    pod.status
        .clone()
        .and_then(|status| status.host_ip)
        .map(|ip| ip.parse::<IpAddr>().ok())
        .and_then(|ip| ip)
}

#[tracing::instrument(skip_all)]
pub async fn run(endpoint: &str, server: CNIServer) {
    if endpoint.contains(".sock") {
        // use UNIX Domain Socket
        tracing::info!(
            "CNI server is started at {} with Unix Domain Socket",
            endpoint
        );
        let sock_file = Path::new(endpoint);
        let uds_listener = UnixListener::bind(sock_file).unwrap();
        let uds_stream = UnixListenerStream::new(uds_listener);
        Server::builder()
            .add_service(CniApiServer::new(server))
            .serve_with_incoming(uds_stream)
            .await
            .unwrap();
    } else {
        tracing::info!("CNI server is started at {} with HTTP", endpoint);
        let sock_addr = endpoint.parse().unwrap();
        Server::builder()
            .add_service(CniApiServer::new(server))
            .serve(sock_addr)
            .await
            .unwrap();
    }
}

#[async_trait]
impl CniApi for CNIServer {
    #[tracing::instrument(skip_all)]
    async fn add(&self, req: Request<Args>) -> Result<Response<CniResult>, Status> {
        let args = req.get_ref();
        tracing::info!(arg=?args, "CNI Add");
        let mut inner = self.inner.lock().await;
        match inner.add(args).await {
            Ok(res) => {
                tracing::info!(result=?res, "Success to add");
                Ok(Response::new(res))
            }
            Err(e) => {
                tracing::error!(error=?e, "Failed to add");
                let error_result = CNIErrorDetail {
                    code: u32::from(&e),
                    msg: e.to_string(),
                    details: e.details(),
                };
                let v = match serde_json::to_vec(&error_result) {
                    Ok(v) => v,
                    Err(e) => {
                        let err = Error::FailedToDecode(e.to_string());
                        let error_result = CNIErrorDetail {
                            code: u32::from(&err),
                            msg: err.to_string(),
                            details: err.details(),
                        };
                        let v = serde_json::to_vec(&error_result).unwrap(); // This must not return error
                        let buf = Bytes::from(v);
                        return Err(Status::with_details(
                            tonic::Code::Internal,
                            "Internal error",
                            buf,
                        ));
                    }
                };
                let buf = Bytes::from(v);
                Err(Status::with_details(
                    tonic::Code::Internal,
                    "Internal error",
                    buf,
                ))
            }
        }
    }

    // https://github.com/containernetworking/cni/blob/main/SPEC.md#del-remove-container-from-network-or-un-apply-modifications
    #[tracing::instrument(skip_all)]
    async fn del(&self, req: Request<Args>) -> Result<Response<CniResult>, Status> {
        let args = req.get_ref();
        tracing::info!(arg=?args, "CNI Del");
        let mut inner = self.inner.lock().await;
        match inner.del(args).await {
            Ok(res) => {
                tracing::info!(result=?res, "Success to delete");
                Ok(Response::new(res))
            }
            Err(e) => {
                tracing::error!(error=?e, "Failed to delete");
                let error_result = CNIErrorDetail {
                    code: u32::from(&e),
                    msg: e.to_string(),
                    details: e.details(),
                };
                let v = match serde_json::to_vec(&error_result) {
                    Ok(v) => v,
                    Err(e) => {
                        let err = Error::FailedToDecode(e.to_string());
                        let error_result = CNIErrorDetail {
                            code: u32::from(&err),
                            msg: err.to_string(),
                            details: err.details(),
                        };
                        let v = serde_json::to_vec(&error_result).unwrap(); // This must not return error
                        let buf = Bytes::from(v);
                        return Err(Status::with_details(
                            tonic::Code::Internal,
                            "Internal error",
                            buf,
                        ));
                    }
                };
                let buf = Bytes::from(v);
                Err(Status::with_details(
                    tonic::Code::Internal,
                    "Internal error",
                    buf,
                ))
            }
        }
    }

    // https://github.com/containernetworking/cni/blob/main/SPEC.md#check-check-containers-networking-is-as-expected
    #[tracing::instrument(skip_all)]
    async fn check(&self, req: Request<Args>) -> Result<Response<CniResult>, Status> {
        let args = req.get_ref();
        tracing::info!(arg=?args, "CNI Check");
        let inner = self.inner.lock().await;
        match inner.check(args).await {
            Ok(res) => {
                tracing::info!(result=?res, "Success to check");
                Ok(Response::new(res))
            }
            Err(e) => {
                tracing::error!(error=?e, "Failed to check");
                let error_result = CNIErrorDetail {
                    code: u32::from(&e),
                    msg: e.to_string(),
                    details: e.details(),
                };
                let v = match serde_json::to_vec(&error_result) {
                    Ok(v) => v,
                    Err(e) => {
                        let err = Error::FailedToDecode(e.to_string());
                        let error_result = CNIErrorDetail {
                            code: u32::from(&err),
                            msg: err.to_string(),
                            details: err.details(),
                        };
                        let v = serde_json::to_vec(&error_result).unwrap(); // This must not return error
                        let buf = Bytes::from(v);
                        return Err(Status::with_details(
                            tonic::Code::Internal,
                            "Internal error",
                            buf,
                        ));
                    }
                };
                let buf = Bytes::from(v);
                Err(Status::with_details(
                    tonic::Code::Internal,
                    "Internal error",
                    buf,
                ))
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct CNIErrorDetail {
    pub(super) code: u32,
    pub(super) msg: String,
    pub(super) details: String,
}

impl From<&CNIErrorDetail> for Error {
    fn from(res: &CNIErrorDetail) -> Self {
        if res.code > 100 {
            return Error::Custom(res.code, res.msg.clone(), res.details.clone());
        }
        match res.code {
            1 => Error::IncompatibleVersion(res.details.clone()),
            2 => Error::UnsupportedNetworkConfiguration(res.details.clone()),
            3 => Error::NotExist(res.details.clone()),
            4 => Error::InvalidEnvValue(res.details.clone()),
            5 => Error::IOFailure(res.details.clone()),
            6 => Error::FailedToDecode(res.details.clone()),
            7 => Error::InvalidNetworkConfig(res.details.clone()),
            11 => Error::TryAgainLater(res.details.clone()),
            _ => Error::FailedToDecode(format!("unknown error code: {}", res.code)),
        }
    }
}
