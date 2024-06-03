use std::{
    collections::HashMap,
    net::{IpAddr, Ipv6Addr},
    path::Path,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use bytes::Bytes;
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use k8s_openapi::api::core::v1::{Namespace, Pod};
use kube::{
    api::{DeleteParams, ListParams, PostParams},
    core::ObjectMeta,
    Api, Client, ResourceExt,
};

use sartd_ipam::manager::{AllocatorSet, Block};
use sartd_proto::sart::{
    cni_api_server::{CniApi, CniApiServer},
    Args, CniResult,
};
use serde::{Deserialize, Serialize};
use tokio::{
    net::UnixListener,
    sync::{mpsc::UnboundedReceiver, Mutex},
};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{async_trait, transport::Server, Request, Response, Status};

use crate::{
    agent::{
        cni::{
            netlink::{self, ContainerLinkInfo},
            netns::{self, get_current_netns, NetNS},
            pod::{cleanup_links, setup_links},
        },
        metrics::Metrics,
    },
    crd::{
        address_block::{AddressBlock, ADDRESS_BLOCK_NODE_LABEL},
        address_pool::{AddressPool, AddressType, ADDRESS_POOL_ANNOTATION},
        block_request::{BlockRequest, BlockRequestSpec},
    },
};

use super::{
    error::Error,
    gc::{self, GarbageCollector},
    pod::{PodAllocation, PodInfo},
};

pub const CNI_SERVER_ENDPOINT: &str = "/var/run/sart.sock";
pub const CNI_ROUTE_TABLE_ID: u32 = 120;

const CONTAINER_INTERFACE_NAME: &str = "eth0";

struct CNIServerInner {
    client: Client,
    allocator: Arc<AllocatorSet>,
    allocation: HashMap<String, PodAllocation>,
    node: String,
    node_addr: IpAddr,
    table: u32,
    receiver: UnboundedReceiver<AddressBlock>,
}

pub struct CNIServer {
    // consider some better type.
    inner: Arc<Mutex<CNIServerInner>>,
    metrics: Arc<std::sync::Mutex<Metrics>>,
}

impl CNIServer {
    pub fn new(
        client: Client,
        allocator: Arc<AllocatorSet>,
        node: String,
        node_addr: IpAddr,
        table: u32,
        receiver: UnboundedReceiver<AddressBlock>,
        metrics: Arc<std::sync::Mutex<Metrics>>,
    ) -> CNIServer {
        CNIServer {
            inner: Arc::new(Mutex::new(CNIServerInner::new(
                client, allocator, node, node_addr, table, receiver,
            ))),
            metrics,
        }
    }

    async fn recover(&mut self) -> Result<(), Error> {
        let inner = self.inner.lock().await;
        inner.recover().await
    }

    async fn garbage_collector(&self, gc_interval: Duration) -> GarbageCollector {
        let inner = self.inner.lock().await;
        inner.garbage_collector(gc_interval)
    }
}

impl CNIServerInner {
    pub fn new(
        client: Client,
        allocator: Arc<AllocatorSet>,
        node: String,
        node_addr: IpAddr,
        table: u32,
        receiver: UnboundedReceiver<AddressBlock>,
    ) -> CNIServerInner {
        CNIServerInner {
            client,
            allocator,
            allocation: HashMap::new(),
            node,
            node_addr,
            table,
            receiver,
        }
    }

    #[tracing::instrument(skip_all)]
    async fn add(&mut self, args: &Args) -> Result<CniResult, Error> {
        let pod_info = PodInfo::from_str(&args.args)?;

        let pod_key = format!("{}/{}", pod_info.namespace, pod_info.name);

        let ns_path = args.netns.clone();
        let ns = NetNS::try_from(ns_path.as_str()).map_err(Error::NetNS)?;

        tracing::info!(
            name = pod_info.name,
            namespace = pod_info.namespace,
            container_id = pod_info.container_id,
            cmd = "ADD",
            "Get pod info"
        );
        let pod_api = Api::<Pod>::namespaced(self.client.clone(), &pod_info.namespace);
        let pod = pod_api.get(&pod_info.name).await.map_err(Error::Kube)?;
        let pool = self.get_pool(&pod_info, &pod).await?;

        if self.allocation.contains_key(&pod_key) {
            return Err(Error::AlreadyConfigured(pod_key));
        }

        let allocated_addr = self.allocate(&pool, &pod_info)?;

        let alloc = match allocated_addr {
            Some(alloc) => {
                tracing::info!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    address = alloc.addr.to_string(),
                    cmd = "ADD",
                    "Allocate addresses"
                );
                alloc
            }
            None => {
                // request
                tracing::info!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
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
                        pool: pool.clone(),
                        node: self.node.clone(),
                    },
                    status: None,
                };
                block_request_api
                    .create(&PostParams::default(), &br)
                    .await
                    .map_err(Error::Kube)?;

                tracing::info!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    cmd = "ADD",
                    "Waiting to create new block"
                );
                // let created_br = self.receiver.blocking_recv();
                let created_br = self.receiver.recv().await.ok_or(Error::ReceiveNotify)?;
                tracing::info!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    cmd = "ADD",
                    "Got the signal of address block creation"
                );

                block_request_api
                    .delete(&br.name_any(), &DeleteParams::default())
                    .await
                    .map_err(Error::Kube)?;
                let alloc = self.allocate_with_block(&created_br.name_any(), &pod_info)?;
                tracing::info!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    address = alloc.addr.to_string(),
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
        let host_ns = get_current_netns().map_err(Error::NetNS)?;
        let allocated_addr = match alloc.addr {
            IpNet::V4(n) => IpNet::V4(Ipv4Net::new(n.addr(), 32).unwrap()),
            IpNet::V6(n) => IpNet::V6(Ipv6Net::new(n.addr(), 128).unwrap()),
        };

        let link_info =
            ContainerLinkInfo::new(&pod_info.container_id, CONTAINER_INTERFACE_NAME, &pool);

        let result = setup_links(
            &link_info,
            &host_ns,
            &ns,
            &allocated_addr,
            &self.node_addr,
            CNI_ROUTE_TABLE_ID,
        )
        .await?;

        Ok(result)
    }

    async fn del(&mut self, args: &Args) -> Result<CniResult, Error> {
        let pod_info = PodInfo::from_str(&args.args)?;
        let pod_api = Api::<Pod>::namespaced(self.client.clone(), &pod_info.namespace);
        // Should we return error here?
        let pod = pod_api.get(&pod_info.name).await.map_err(Error::Kube)?;
        let addr_opt = get_pod_addr(&pod);

        if let Err(e) = self.get_pool(&pod_info, &pod).await {
            match e {
                Error::DefaultPoolNotFound => {
                    tracing::warn!(
                        name = pod_info.name,
                        namespace = pod_info.namespace,
                        container_id = pod_info.container_id,
                        cmd = "DEL",
                        "Default pool is not found"
                    );
                    return Ok(CniResult {
                        interfaces: vec![],
                        ips: vec![],
                        routes: vec![],
                        dns: None,
                    });
                }
                _ => return Err(e),
            }
        }
        if addr_opt.is_none() {
            tracing::warn!(
                name = pod_info.name,
                namespace = pod_info.namespace,
                container_id = pod_info.container_id,
                cmd = "DEL",
                "Pod's address doesn't exist"
            );
        }
        let alloc_opt = match self.get_allocation(&pod_info) {
            Ok(alloc_opt) => alloc_opt,
            Err(_e) => {
                tracing::warn!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    cmd = "DEL",
                    "container-id is not matched with actual stored allocation"
                );
                None
            }
        };
        if alloc_opt.is_none() {
            tracing::warn!(
                name = pod_info.name,
                namespace = pod_info.namespace,
                container_id = pod_info.container_id,
                cmd = "DEL",
                "Allocation information doesn't exist"
            );
        }

        let mut deletable_block = None;

        // get actual assigned address in pod netns
        let result = match alloc_opt {
            Some(alloc) => match self.release(&alloc.block, &alloc.addr.addr()) {
                Ok(is_empty) => {
                    tracing::info!(
                        name = pod_info.name,
                        namespace = pod_info.namespace,
                        container_id = pod_info.container_id,
                        address = alloc.addr.to_string(),
                        empty = is_empty,
                        cmd = "DEL",
                        "Release allocated address"
                    );
                    self.allocation
                        .remove(&format!("{}/{}", pod_info.namespace, pod_info.name));
                    if is_empty {
                        deletable_block = Some(alloc.block.clone());
                    }

                    let host_ns = get_current_netns().map_err(Error::NetNS)?;
                    let container_ns =
                        match NetNS::try_from(args.netns.as_str()).map_err(Error::NetNS) {
                            Ok(ns) => Ok(Some(ns)),
                            Err(Error::NetNS(e)) => match e {
                                netns::Error::NotExist(_) => Ok(None),
                                _ => Err(Error::NetNS(e)),
                            },
                            Err(e) => Err(e),
                        }?;

                    match cleanup_links(
                        &pod_info.container_id,
                        &host_ns,
                        container_ns,
                        &alloc.addr,
                        &self.node_addr,
                        CONTAINER_INTERFACE_NAME,
                        CNI_ROUTE_TABLE_ID,
                    )
                    .await
                    {
                        Ok(result) => result,
                        Err(e) => match e {
                            Error::Netlink(e) => match e {
                                netlink::Error::LinkNotFound(_) => {
                                    tracing::warn!(
                                        name = pod_info.name,
                                        namespace = pod_info.namespace,
                                        container_id = pod_info.container_id,
                                        address = alloc.addr.to_string(),
                                        cmd = "DEL",
                                        "{e}",
                                    );
                                    CniResult {
                                        interfaces: vec![],
                                        ips: vec![],
                                        routes: vec![],
                                        dns: None,
                                    }
                                }
                                netlink::Error::RouteNotFound(_) => {
                                    tracing::warn!(
                                        name = pod_info.name,
                                        namespace = pod_info.namespace,
                                        container_id = pod_info.container_id,
                                        address = alloc.addr.to_string(),
                                        cmd = "DEL",
                                        "{e}",
                                    );
                                    CniResult {
                                        interfaces: vec![],
                                        ips: vec![],
                                        routes: vec![],
                                        dns: None,
                                    }
                                }
                                _ => return Err(Error::Netlink(e)),
                            },
                            _ => return Err(e),
                        },
                    }
                }
                Err(e) => {
                    tracing::error!(
                        name = pod_info.name,
                        namespace = pod_info.namespace,
                        container_id = pod_info.container_id,
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

        // If the block is not be used, remove it.
        // This operation must not affect the result of CNI Del command.
        if let Some(block) = deletable_block {
            tracing::info!(
                name = pod_info.name,
                namespace = pod_info.namespace,
                container_id = pod_info.container_id,
                block = block,
                cmd = "DEL",
                "Block is unused. Delete this block."
            );
            let address_block_api = Api::<AddressBlock>::all(self.client.clone());
            if let Err(err) = address_block_api
                .delete(&block, &DeleteParams::default())
                .await
                .map_err(Error::Kube)
            {
                tracing::error!(
                    name = pod_info.name,
                    namespace = pod_info.namespace,
                    container_id = pod_info.container_id,
                    block = block,
                    error=?err,
                    cmd = "DEL",
                    "Failed to delete unused address block",

                )
            }
        }

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
        let pod = pod_api.get(&pod_info.name).await.map_err(Error::Kube)?;
        let pool = self.get_pool(&pod_info, &pod).await?;
        let pod_addr = get_pod_addr(&pod).ok_or(Error::PodAddressIsNotFound)?;
        let alloc = self
            .get_allocation(&pod_info)?
            .ok_or(Error::AllocationNotFound)?;
        if alloc.addr.addr().ne(&pod_addr) {
            return Err(Error::AddressNotMatched);
        }

        if !self.is_allocated(&pool, &alloc.addr.addr()) {
            return Err(Error::AllocationNotFound);
        }

        Ok(CniResult {
            interfaces: Vec::new(),
            ips: Vec::new(),
            routes: Vec::new(),
            dns: None,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn recover(&self) -> Result<(), Error> {
        tracing::info!("Recover existing allocation");

        let link_list = netlink::link_list().await.map_err(Error::Netlink)?;
        let route4_list = netlink::route_list(IpAddr::from_str("127.0.0.1").unwrap(), self.table)
            .await
            .map_err(Error::Netlink)?;
        let route6_list = netlink::route_list(IpAddr::from_str("fe80::1").unwrap(), self.table)
            .await
            .map_err(Error::Netlink)?;

        let address_block_api = Api::<AddressBlock>::all(self.client.clone());
        let list_params =
            ListParams::default().labels(&format!("{}={}", ADDRESS_BLOCK_NODE_LABEL, self.node));
        let address_block_list = address_block_api
            .list(&list_params)
            .await
            .map_err(Error::Kube)?;

        let mut pool_block_map: HashMap<String, Vec<String>> = HashMap::new();

        for block in address_block_list.iter() {
            if block.spec.r#type.ne(&AddressType::Pod) {
                continue;
            }
            match pool_block_map.get_mut(&block.spec.pool_ref) {
                Some(block_list) => block_list.push(block.name_any()),
                None => {
                    pool_block_map.insert(block.spec.pool_ref.clone(), vec![block.name_any()]);
                }
            }
        }

        {
            let alloc_set = self.allocator.clone();
            let mut allocator = alloc_set.inner.lock().map_err(|_| Error::Lock)?;

            for ab in address_block_list.iter() {
                match allocator.get_mut(&ab.name_any()) {
                    Some(_block) => {
                        tracing::info!(
                            pool_name = ab.spec.pool_ref,
                            block_name = ab.name_any(),
                            "Block already exists"
                        );
                    }
                    None => {
                        let cidr = IpNet::from_str(ab.spec.cidr.as_str())
                            .map_err(|_| Error::InvalidAddress(ab.spec.cidr.clone()))?;
                        let block = Block::new(ab.name_any(), ab.spec.pool_ref.clone(), cidr)
                            .map_err(Error::Ipam)?;
                        tracing::info!(
                            pool_name = ab.spec.pool_ref,
                            block_name = ab.name_any(),
                            "Register existing address block"
                        );
                        allocator.insert(block, false).map_err(Error::Ipam)?;
                    }
                }
            }

            // Register existing addresses to the allocator
            for (ifname, link_info) in link_list.iter() {
                let addr_opt = match route4_list.get(ifname) {
                    Some(addr) => Some(*addr),
                    None => route6_list.get(ifname).copied(),
                };
                if let Some(addr) = addr_opt {
                    // Register
                    if let Some(block_names) = pool_block_map.get(&link_info.pool) {
                        for block_name in block_names.iter() {
                            if let Some(block) = allocator.get_mut(block_name) {
                                if block.allocator.cidr().contains(&addr) {
                                    // Recover existing address
                                    tracing::info!(
                                        pool_name = link_info.pool,
                                        block_name = block_name,
                                        address =? addr,
                                        container_id = link_info.id,
                                        "Recover the existing allocation"
                                    );
                                    block.allocator.allocate(&addr, true).map_err(Error::Ipam)?;
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn garbage_collector(&self, gc_interval: Duration) -> GarbageCollector {
        gc::GarbageCollector::new(gc_interval, self.client.clone(), self.allocator.clone())
    }

    async fn get_pool(&self, pod_info: &PodInfo, pod: &Pod) -> Result<String, Error> {
        let namespace_api = Api::<Namespace>::all(self.client.clone());
        let address_pool_api = Api::<AddressPool>::all(self.client.clone());

        let pool_opt = match pod.annotations().get(ADDRESS_POOL_ANNOTATION) {
            Some(pool) => Some(pool.to_string()),
            None => {
                let ns = namespace_api
                    .get(&pod_info.namespace)
                    .await
                    .map_err(Error::Kube)?;
                ns.annotations().get(ADDRESS_POOL_ANNOTATION).cloned()
            }
        };
        let pool = match pool_opt {
            Some(pool) => pool,
            None => {
                let default = {
                    let allocator = self.allocator.inner.lock().map_err(|_| Error::Lock)?;
                    allocator.auto_assign.clone()
                };
                match default {
                    Some(pool) => pool,
                    None => {
                        // In case of completely first pod allocation,
                        let ap_list = address_pool_api
                            .list(&ListParams::default())
                            .await
                            .map_err(Error::Kube)?;
                        match ap_list.into_iter().find(|p| {
                            p.spec.auto_assign.unwrap_or(false)
                                && p.spec.r#type.eq(&AddressType::Pod)
                        }) {
                            Some(default_pool) => default_pool.name_any(),
                            None => return Err(Error::DefaultPoolNotFound),
                        }
                    }
                }
            }
        };

        Ok(pool)
    }

    fn get_allocation(&self, pod_info: &PodInfo) -> Result<Option<PodAllocation>, Error> {
        match self
            .allocation
            .get(&format!("{}/{}", pod_info.namespace, pod_info.name))
            .cloned()
        {
            Some(alloc) => {
                if alloc.container_id.ne(&pod_info.container_id) {
                    return Err(Error::AllocationNotFound);
                }
                Ok(Some(alloc))
            }
            None => Ok(None),
        }
    }

    fn allocate(&mut self, pool: &str, pod_info: &PodInfo) -> Result<Option<PodAllocation>, Error> {
        let mut allocator = self.allocator.inner.lock().map_err(|_| Error::Lock)?;
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
                let addr = block.allocator.allocate_next().map_err(Error::Ipam)?;
                let cidr = IpNet::new(addr, prefix)
                    .map_err(|_| Error::InvalidAddress(format!("{}/{}", addr, prefix)))?;
                return Ok(Some(PodAllocation {
                    container_id: pod_info.container_id.clone(),
                    block: block.name.clone(),
                    addr: cidr,
                }));
            }
        }
        Ok(None)
    }

    fn allocate_with_block(
        &mut self,
        name: &str,
        pod_info: &PodInfo,
    ) -> Result<PodAllocation, Error> {
        let mut allocator = self.allocator.inner.lock().map_err(|_| Error::Lock)?;
        let block = allocator
            .blocks
            .get_mut(name)
            .ok_or(Error::BlockNotFound(name.to_string()))?;
        let prefix = block.allocator.prefix_len();
        let addr = block.allocator.allocate_next().map_err(Error::Ipam)?;
        let cidr = IpNet::new(addr, prefix)
            .map_err(|_| Error::InvalidAddress(format!("{}/{}", addr, prefix)))?;
        Ok(PodAllocation {
            container_id: pod_info.container_id.clone(),
            block: name.to_string(),
            addr: cidr,
        })
    }

    fn release(&mut self, name: &str, addr: &IpAddr) -> Result<bool, Error> {
        let mut allocator = self.allocator.inner.lock().map_err(|_| Error::Lock)?;
        let block = allocator
            .blocks
            .get_mut(name)
            .ok_or(Error::BlockNotFound(name.to_string()))?;
        block.allocator.release(addr).map_err(Error::Ipam)?;

        let empty = block.allocator.is_empty();

        Ok(empty)
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
pub async fn run(endpoint: &str, mut server: CNIServer) {
    tracing::info!(
        table = CNI_ROUTE_TABLE_ID,
        "Initialize new routing rule in kernel"
    );
    if netlink::get_rule(CNI_ROUTE_TABLE_ID, &IpAddr::V4([127, 0, 0, 1].into()))
        .await
        .unwrap()
        .is_none()
    {
        netlink::add_rule(CNI_ROUTE_TABLE_ID, IpAddr::V4([127, 0, 0, 1].into()))
            .await
            .unwrap();
    }
    if netlink::get_rule(
        CNI_ROUTE_TABLE_ID,
        &IpAddr::V6(Ipv6Addr::from_str("fe80::1").unwrap()),
    )
    .await
    .unwrap()
    .is_none()
    {
        netlink::add_rule(
            CNI_ROUTE_TABLE_ID,
            IpAddr::V6(Ipv6Addr::from_str("fe80::1").unwrap()),
        )
        .await
        .unwrap();
    }

    server.recover().await.unwrap();

    let mut garbage_collector = server.garbage_collector(Duration::from_secs(60)).await;

    tracing::info!("Start Garbage collector");
    tokio::spawn(async move {
        garbage_collector.run().await;
    });

    if endpoint.contains(".sock") {
        // use UNIX Domain Socket
        // FIXME: gRPC server via UNIX Domain Socket doesn't work
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
        let mut inner = self.inner.lock().await;
        match inner.add(args).await {
            Ok(res) => {
                if let Ok(metrics) = self.metrics.lock() {
                    metrics.cni_call("add");
                }
                Ok(Response::new(res))
            }
            Err(e) => {
                tracing::error!(error=?e, "Failed to add");
                let cni_err = rscni::error::Error::from(e);
                let error_result = CNIErrorDetail {
                    code: u32::from(&cni_err),
                    msg: cni_err.to_string(),
                    details: cni_err.details(),
                };
                if let Ok(metrics) = self.metrics.lock() {
                    metrics.cni_errors("add", &format!("{}", u32::from(&cni_err)));
                }
                let v = match serde_json::to_vec(&error_result) {
                    Ok(v) => v,
                    Err(e) => {
                        let err = rscni::error::Error::FailedToDecode(e.to_string());
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
        let mut inner = self.inner.lock().await;
        match inner.del(args).await {
            Ok(res) => {
                if let Ok(metrics) = self.metrics.lock() {
                    metrics.cni_call("del");
                }
                Ok(Response::new(res))
            }
            Err(e) => {
                tracing::error!(error=?e, "Failed to delete");
                let cni_err = rscni::error::Error::from(e);
                let error_result = CNIErrorDetail {
                    code: u32::from(&cni_err),
                    msg: cni_err.to_string(),
                    details: cni_err.details(),
                };
                if let Ok(metrics) = self.metrics.lock() {
                    metrics.cni_errors("del", &format!("{}", u32::from(&cni_err)));
                }
                let v = match serde_json::to_vec(&error_result) {
                    Ok(v) => v,
                    Err(e) => {
                        let err = rscni::error::Error::FailedToDecode(e.to_string());
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
        let inner = self.inner.lock().await;
        match inner.check(args).await {
            Ok(res) => {
                if let Ok(metrics) = self.metrics.lock() {
                    metrics.cni_call("check");
                }
                Ok(Response::new(res))
            }
            Err(e) => {
                tracing::error!(error=?e, "Failed to check");
                let cni_err = rscni::error::Error::from(e);
                let error_result = CNIErrorDetail {
                    code: u32::from(&cni_err),
                    msg: cni_err.to_string(),
                    details: cni_err.details(),
                };
                if let Ok(metrics) = self.metrics.lock() {
                    metrics.cni_errors("check", &format!("{}", u32::from(&cni_err)));
                }
                let v = match serde_json::to_vec(&error_result) {
                    Ok(v) => v,
                    Err(e) => {
                        let err = rscni::error::Error::FailedToDecode(e.to_string());
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

impl From<&CNIErrorDetail> for rscni::error::Error {
    fn from(res: &CNIErrorDetail) -> Self {
        if res.code > 100 {
            return Self::Custom(res.code, res.msg.clone(), res.details.clone());
        }
        match res.code {
            1 => Self::IncompatibleVersion(res.details.clone()),
            2 => Self::UnsupportedNetworkConfiguration(res.details.clone()),
            3 => Self::NotExist(res.details.clone()),
            4 => Self::InvalidEnvValue(res.details.clone()),
            5 => Self::IOFailure(res.details.clone()),
            6 => Self::FailedToDecode(res.details.clone()),
            7 => Self::InvalidNetworkConfig(res.details.clone()),
            11 => Self::TryAgainLater(res.details.clone()),
            _ => Self::FailedToDecode(format!("unknown error code: {}", res.code)),
        }
    }
}

const CNI_ERROR_CODE_KUBE: u32 = 200;
const CNI_ERROR_MSG_KUBE: &str = "Kubernetes error";
const CNI_ERROR_CODE_INTERNAL: u32 = 210;
const CNI_ERROR_MSG_INTERNAL: &str = "Internal error";
const CNI_ERROR_CODE_ALLOCATOR: u32 = 220;
const CNI_ERROR_MSG_ALLOCATOR: &str = "Allocation error";
const CNI_ERROR_CODE_NETWORK_CONFIG: u32 = 230;
const CNI_ERROR_MSG_NETWORK_CONFIG: &str = "Network configuration error";
const CNI_ERROR_CODE_IPAM: u32 = 240;
const CNI_ERROR_MSG_IPAM: &str = "Ipam error";
const CNI_ERROR_CODE_NETNS: u32 = 250;
const CNI_ERROR_MSG_NETNS: &str = "NetNS error";
const CNI_ERROR_CODE_NETLINK: u32 = 260;
const CNI_ERROR_MSG_NETLINK: &str = "Netlink error";

impl From<Error> for rscni::error::Error {
    fn from(value: Error) -> Self {
        match value {
            Error::AddressNotMatched => Self::Custom(
                CNI_ERROR_CODE_NETWORK_CONFIG,
                CNI_ERROR_MSG_NETWORK_CONFIG.to_string(),
                value.to_string(),
            ),
            Error::AllocationNotFound => Self::Custom(
                CNI_ERROR_CODE_ALLOCATOR,
                CNI_ERROR_MSG_ALLOCATOR.to_string(),
                value.to_string(),
            ),
            Error::BlockNotFound(detail) => Self::Custom(
                CNI_ERROR_CODE_ALLOCATOR,
                CNI_ERROR_MSG_ALLOCATOR.to_string(),
                detail,
            ),
            Error::DefaultPoolNotFound => {
                Self::TryAgainLater("default pool is not found".to_string())
            }
            Error::AlreadyConfigured(key) => Self::Custom(
                CNI_ERROR_CODE_INTERNAL,
                CNI_ERROR_MSG_INTERNAL.to_string(),
                key,
            ),
            Error::InvalidAddress(detail) => Self::Custom(
                CNI_ERROR_CODE_NETWORK_CONFIG,
                CNI_ERROR_MSG_NETWORK_CONFIG.to_string(),
                detail,
            ),
            Error::Ipam(e) => Self::Custom(
                CNI_ERROR_CODE_IPAM,
                CNI_ERROR_MSG_IPAM.to_string(),
                e.to_string(),
            ),
            Error::Kube(e) => Self::Custom(
                CNI_ERROR_CODE_KUBE,
                CNI_ERROR_MSG_KUBE.to_string(),
                e.to_string(),
            ),
            Error::Lock => Self::Custom(
                CNI_ERROR_CODE_INTERNAL,
                CNI_ERROR_MSG_INTERNAL.to_string(),
                value.to_string(),
            ),
            Error::MissingField(detail) => Self::Custom(
                CNI_ERROR_CODE_INTERNAL,
                CNI_ERROR_MSG_INTERNAL.to_string(),
                detail,
            ),
            Error::NetNS(e) => Self::Custom(
                CNI_ERROR_CODE_NETNS,
                CNI_ERROR_MSG_NETNS.to_string(),
                e.to_string(),
            ),
            Error::Netlink(e) => Self::Custom(
                CNI_ERROR_CODE_NETLINK,
                CNI_ERROR_MSG_NETLINK.to_string(),
                e.to_string(),
            ),
            Error::PodAddressIsNotFound => Self::Custom(
                CNI_ERROR_CODE_INTERNAL,
                CNI_ERROR_MSG_INTERNAL.to_string(),
                value.to_string(),
            ),
            Error::ReceiveNotify => Self::Custom(
                CNI_ERROR_CODE_INTERNAL,
                CNI_ERROR_MSG_INTERNAL.to_string(),
                value.to_string(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::IpAddr,
        str::FromStr,
        sync::{Arc, Mutex},
    };

    use ipnet::IpNet;
    use kube::Client;
    use sartd_ipam::manager::{AllocatorSet, Block};
    use tokio::sync::mpsc::unbounded_channel;

    use crate::agent::{
        cni::{
            pod::{PodAllocation, PodInfo},
            server::{CNIServerInner, CNI_ROUTE_TABLE_ID},
        },
        metrics::Metrics,
    };

    #[tokio::test]
    async fn test_cni_server_allocate() {
        // set dummy kube config path
        std::env::set_var("KUBECONFIG", "tests/config/dummy_kubeconfig");
        let dummy_client = Client::try_default().await.unwrap();
        let allocator = Arc::new(AllocatorSet::new());
        let (_sender, receiver) = unbounded_channel();
        let mut inner_cni_server = CNIServerInner::new(
            dummy_client,
            allocator.clone(),
            "test-node".to_string(),
            IpAddr::from_str("127.0.0.1").unwrap(),
            CNI_ROUTE_TABLE_ID,
            receiver,
        );

        let pod1 = PodInfo {
            container_id: "pod1-container-id".to_string(),
            uid: "pod1-uid".to_string(),
            namespace: "default".to_string(),
            name: "pod1".to_string(),
        };

        // If pool doesn't exist, allocate() should return None without any error.
        let should_none = inner_cni_server.allocate("pool1", &pod1).unwrap();
        assert!(should_none.is_none());

        // register pool
        let pool1_block1 = Block::new(
            "pool1_block1".to_string(),
            "pool1".to_string(),
            IpNet::from_str("10.0.0.0/31").unwrap(),
        )
        .unwrap();
        {
            let alloc = allocator.clone();
            let mut tmp_allocator = alloc.inner.lock().unwrap();
            tmp_allocator.insert(pool1_block1, false).unwrap();
        }

        let should_some = inner_cni_server.allocate("pool1", &pod1).unwrap();
        assert!(should_some.is_some());
        let res = should_some.unwrap();
        assert_eq!(
            res,
            PodAllocation {
                container_id: pod1.container_id.clone(),
                block: "pool1_block1".to_string(),
                addr: IpNet::from_str("10.0.0.0/31").unwrap()
            }
        );

        // create second pod
        let should_some = inner_cni_server.allocate("pool1", &pod1).unwrap();
        assert!(should_some.is_some());
        let res = should_some.unwrap();
        assert_eq!(
            res,
            PodAllocation {
                container_id: pod1.container_id.clone(),
                block: "pool1_block1".to_string(),
                addr: IpNet::from_str("10.0.0.1/31").unwrap()
            }
        );
        // When creating the third pod, specified block is full.
        // In this case, allocate() returns None without any error.
        let should_none = inner_cni_server.allocate("pool1", &pod1).unwrap();
        assert!(should_none.is_none());

        // adding new block for pool1
        let pool1_block2 = Block::new(
            "pool1_block2".to_string(),
            "pool1".to_string(),
            IpNet::from_str("10.0.0.2/31").unwrap(),
        )
        .unwrap();
        {
            let alloc = allocator.clone();
            let mut tmp_allocator = alloc.inner.lock().unwrap();
            tmp_allocator.insert(pool1_block2, false).unwrap();
        }
        let should_some = inner_cni_server.allocate("pool1", &pod1).unwrap();
        assert!(should_some.is_some());
        let res = should_some.unwrap();

        // When creating fourth pod, the address should be allocated from pool1_block2
        assert_eq!(
            res,
            PodAllocation {
                container_id: pod1.container_id.clone(),
                block: "pool1_block2".to_string(),
                addr: IpNet::from_str("10.0.0.2/31").unwrap()
            }
        );
        // adding new block for another pool(pool2)
        let pool2_block1 = Block::new(
            "pool2_block1".to_string(),
            "pool2".to_string(),
            IpNet::from_str("10.0.1.0/32").unwrap(),
        )
        .unwrap();
        {
            let alloc = allocator.clone();
            let mut tmp_allocator = alloc.inner.lock().unwrap();
            tmp_allocator.insert(pool2_block1, false).unwrap();
        }
        let should_some = inner_cni_server.allocate("pool2", &pod1).unwrap();
        assert!(should_some.is_some());
        let res = should_some.unwrap();

        // When creating fourth pod, the address should be allocated from pool1_block2
        assert_eq!(
            res,
            PodAllocation {
                container_id: pod1.container_id.clone(),
                block: "pool2_block1".to_string(),
                addr: IpNet::from_str("10.0.1.0/32").unwrap()
            }
        );
    }

    #[tokio::test]
    async fn test_cni_server_allocate_with_block() {
        // set dummy kube config path
        std::env::set_var("KUBECONFIG", "tests/config/dummy_kubeconfig");
        let dummy_client = Client::try_default().await.unwrap();
        let allocator = Arc::new(AllocatorSet::new());
        let (_sender, receiver) = unbounded_channel();
        let mut inner_cni_server = CNIServerInner::new(
            dummy_client,
            allocator.clone(),
            "test-node".to_string(),
            IpAddr::from_str("127.0.0.1").unwrap(),
            CNI_ROUTE_TABLE_ID,
            receiver,
        );

        let pod1 = PodInfo {
            container_id: "pod1-container-id".to_string(),
            uid: "pod1-uid".to_string(),
            namespace: "default".to_string(),
            name: "pod1".to_string(),
        };

        // If pool doesn't exist, allocate() should return None without any error.
        let should_err = inner_cni_server.allocate_with_block("pool1_block1", &pod1);
        // expect BlockNotFound error
        assert!(should_err.is_err());

        // register pool
        let pool1_block1 = Block::new(
            "pool1_block1".to_string(),
            "pool1".to_string(),
            IpNet::from_str("10.0.0.0/31").unwrap(),
        )
        .unwrap();
        {
            let alloc = allocator.clone();
            let mut tmp_allocator = alloc.inner.lock().unwrap();
            tmp_allocator.insert(pool1_block1, false).unwrap();
        }

        let res = inner_cni_server
            .allocate_with_block("pool1_block1", &pod1)
            .unwrap();
        assert_eq!(
            res,
            PodAllocation {
                container_id: pod1.container_id.clone(),
                block: "pool1_block1".to_string(),
                addr: IpNet::from_str("10.0.0.0/31").unwrap()
            }
        );

        // allocating to the second pod
        let res = inner_cni_server
            .allocate_with_block("pool1_block1", &pod1)
            .unwrap();
        assert_eq!(
            res,
            PodAllocation {
                container_id: pod1.container_id.clone(),
                block: "pool1_block1".to_string(),
                addr: IpNet::from_str("10.0.0.1/31").unwrap()
            }
        );

        let should_err_full = inner_cni_server.allocate_with_block("pool1_block1", &pod1);
        // expect Ipam::Full error
        assert!(should_err_full.is_err());
    }

    #[tokio::test]
    async fn test_cni_server_release() {
        std::env::set_var("KUBECONFIG", "tests/config/dummy_kubeconfig");
        let dummy_client = Client::try_default().await.unwrap();
        let allocator = Arc::new(AllocatorSet::new());
        let (_sender, receiver) = unbounded_channel();
        let mut inner_cni_server = CNIServerInner::new(
            dummy_client,
            allocator.clone(),
            "test-node".to_string(),
            IpAddr::from_str("127.0.0.1").unwrap(),
            CNI_ROUTE_TABLE_ID,
            receiver,
        );

        let pod1 = PodInfo {
            container_id: "pod1-container-id".to_string(),
            uid: "pod1-uid".to_string(),
            namespace: "default".to_string(),
            name: "pod1".to_string(),
        };

        // If pool doesn't exist, allocate() should return None without any error.
        let should_none = inner_cni_server.allocate("pool1", &pod1).unwrap();
        assert!(should_none.is_none());

        // register pool
        let pool1_block1 = Block::new(
            "pool1_block1".to_string(),
            "pool1".to_string(),
            IpNet::from_str("10.0.0.0/31").unwrap(),
        )
        .unwrap();
        {
            let alloc = allocator.clone();
            let mut tmp_allocator = alloc.inner.lock().unwrap();
            tmp_allocator.insert(pool1_block1, false).unwrap();
        }

        let should_some = inner_cni_server.allocate("pool1", &pod1).unwrap();
        assert!(should_some.is_some());
        let res = should_some.unwrap();
        assert_eq!(
            res,
            PodAllocation {
                container_id: pod1.container_id.clone(),
                block: "pool1_block1".to_string(),
                addr: IpNet::from_str("10.0.0.0/31").unwrap()
            }
        );

        // create second pod
        let should_some = inner_cni_server.allocate("pool1", &pod1).unwrap();
        assert!(should_some.is_some());
        let res = should_some.unwrap();
        assert_eq!(
            res,
            PodAllocation {
                container_id: pod1.container_id.clone(),
                block: "pool1_block1".to_string(),
                addr: IpNet::from_str("10.0.0.1/31").unwrap()
            }
        );
        // When creating the third pod, specified block is full.
        // In this case, allocate() returns None without any error.
        let should_none = inner_cni_server.allocate("pool1", &pod1).unwrap();
        assert!(should_none.is_none());

        // adding new block for pool1
        let pool1_block2 = Block::new(
            "pool1_block2".to_string(),
            "pool1".to_string(),
            IpNet::from_str("10.0.0.2/31").unwrap(),
        )
        .unwrap();
        {
            let alloc = allocator.clone();
            let mut tmp_allocator = alloc.inner.lock().unwrap();
            tmp_allocator.insert(pool1_block2, false).unwrap();
        }
        let should_some = inner_cni_server.allocate("pool1", &pod1).unwrap();
        assert!(should_some.is_some());
        let res = should_some.unwrap();

        // When creating fourth pod, the address should be allocated from pool1_block2
        assert_eq!(
            res,
            PodAllocation {
                container_id: pod1.container_id.clone(),
                block: "pool1_block2".to_string(),
                addr: IpNet::from_str("10.0.0.2/31").unwrap()
            }
        );
        // adding new block for another pool(pool2)
        let pool2_block1 = Block::new(
            "pool2_block1".to_string(),
            "pool2".to_string(),
            IpNet::from_str("10.0.1.0/32").unwrap(),
        )
        .unwrap();
        {
            let alloc = allocator.clone();
            let mut tmp_allocator = alloc.inner.lock().unwrap();
            tmp_allocator.insert(pool2_block1, false).unwrap();
        }
        let should_some = inner_cni_server.allocate("pool2", &pod1).unwrap();
        assert!(should_some.is_some());
        let res = should_some.unwrap();

        // When creating fourth pod, the address should be allocated from pool1_block2
        assert_eq!(
            res,
            PodAllocation {
                container_id: pod1.container_id.clone(),
                block: "pool2_block1".to_string(),
                addr: IpNet::from_str("10.0.1.0/32").unwrap()
            }
        );

        // check the address is allocated
        {
            let tmp_allocator_set = allocator.clone();
            let tmp_allocator = tmp_allocator_set.inner.lock().unwrap();
            assert!(tmp_allocator
                .blocks
                .get("pool2_block1")
                .unwrap()
                .allocator
                .is_allocated(&IpAddr::from_str("10.0.1.0").unwrap()));
        }
        inner_cni_server
            .release("pool2_block1", &IpAddr::from_str("10.0.1.0").unwrap())
            .unwrap();
        // check the address is released
        {
            let tmp_allocator_set = allocator.clone();
            let tmp_allocator = tmp_allocator_set.inner.lock().unwrap();
            assert!(!tmp_allocator
                .blocks
                .get("pool2_block1")
                .unwrap()
                .allocator
                .is_allocated(&IpAddr::from_str("10.0.1.0").unwrap()));
        }
        // try to release the address already released again.
        let should_err =
            inner_cni_server.release("pool2_block1", &IpAddr::from_str("10.0.1.0").unwrap());
        // expect Ipam::NoReleasableAddress error
        assert!(should_err.is_err());
    }
}
