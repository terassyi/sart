use std::{collections::HashMap, net::IpAddr, str::FromStr, sync::Arc};

use futures::StreamExt;
use k8s_openapi::api::{
    core::v1::{LoadBalancerIngress, LoadBalancerStatus, Service, ServiceStatus},
    discovery::v1::EndpointSlice,
};
use kube::{
    api::{ListParams, PostParams},
    runtime::{
        controller::Action,
        finalizer::{finalizer, Event},
        watcher::Config,
        Controller,
    },
    Api, Client, ResourceExt,
};

use crate::{
    context::{error_policy, ContextWith, Ctx, State},
    controller::error::Error,
    crd::address_pool::{ADDRESS_POOL_ANNOTATION, LOADBALANCER_ADDRESS_ANNOTATION},
    util::{diff, get_namespace},
};
use sartd_ipam::manager::{AllocatorSet, Block};

pub const SERVICE_FINALIZER: &str = "service.sart.terassyi.net/finalizer";
pub(super) const SERVICE_NAME_LABEL: &str = "kubernetes.io/service-name";
// This annoation should be annotated to Endpointslices to handle externalTrafficPolicy
// This annotation is used only for triggering the Endpoinslice's reconciliation loop
pub const SERVICE_ETP_ANNOTATION: &str = "service.sart.terassyi.net/etp";
pub const SERVICE_ALLOCATION_ANNOTATION: &str = "service.sart.terassyi.net/allocation";
pub const RELEASE_ANNOTATION: &str = "service.sart.terassyi.net/release";

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(
    svc: Arc<Service>,
    ctx: Arc<ContextWith<Arc<AllocatorSet>>>,
) -> Result<Action, Error> {
    let ns = get_namespace::<Service>(&svc).map_err(Error::KubeLibrary)?;

    let services = Api::<Service>::namespaced(ctx.client().clone(), &ns);

    finalizer(&services, SERVICE_FINALIZER, svc, |event| async {
        match event {
            Event::Apply(svc) => reconcile(&services, &svc, ctx.clone()).await,
            Event::Cleanup(svc) => cleanup(&services, &svc, ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(
    api: &Api<Service>,
    svc: &Service,
    ctx: Arc<ContextWith<Arc<AllocatorSet>>>,
) -> Result<Action, Error> {
    let ns = get_namespace::<Service>(svc).map_err(Error::KubeLibrary)?;

    tracing::info!(name = svc.name_any(), namespace = ns, "Reconcile Service");

    if !is_loadbalancer(svc) {
        // If Service is not LoadBalancer and it has allocated load balancer addresses, release these.
        // Once Service resources are changed from LoadBalance to other types,
        // the status field of it will be removed immediately.
        // So, for propagating addresses we have to release, we will add an annotation "service.sart.terassyi.net/release".
        // When reconciler is triggered with some service that is not LoadBalancer,
        // we check its annotation.
        // And if it has, we have to release its addresses.
        let releasable_addrs = get_releasable_addrs(svc);
        tracing::info!(name = svc.name_any(), namespace = ns, releasable=?releasable_addrs, "not LoadBalancer");
        if let Some(addrs) = releasable_addrs {
            let released = {
                let c = ctx.component.clone();
                release_lb_addrs(svc, &addrs, &c)?
            };
            tracing::warn!(
                name = svc.name_any(),
                namespace = ns,
                lb_addrs=?released,
                "Release lb addresses because this service is not a load balancer");
        }
        return Ok(Action::await_change());
    }

    // In case of Service is LoadBalancer type

    // Ensure that some blocks are available.
    let block_available = {
        let c = ctx.component.clone();
        let alloc_set = c.inner.lock().map_err(|_| Error::FailedToGetLock)?;
        !alloc_set.blocks.is_empty()
    };
    if !block_available {
        tracing::warn!(
            name = svc.name_any(),
            namespace = ns,
            "Any address blocks is not set."
        );
        return Err(Error::NoAllocatableBlock);
    }

    // Get already allocated addresses
    let actual_addrs = get_allocated_lb_addrs(svc).unwrap_or_default();

    let allocated_addrs = actual_addrs
        .iter()
        .map(|a| a.to_string())
        .collect::<Vec<String>>()
        .join(",");
    let allocated_addr_str = allocated_addrs.as_str();

    // Sync the externalTrafficPolicy value and allocation
    let etp = get_etp(svc).ok_or(Error::InvalidExternalTrafficPolicy)?;

    let endpointslice_api = Api::<EndpointSlice>::namespaced(ctx.client().clone(), &ns);
    let lp = ListParams::default().labels(&format!("{}={}", SERVICE_NAME_LABEL, svc.name_any()));
    let epss = endpointslice_api.list(&lp).await.map_err(Error::Kube)?;

    for eps in epss.items.iter() {
        let mut need_update = false;
        // sync externalTrafficPolicy
        let mut new_eps = eps.clone();
        let new_etp = match new_eps.annotations().get(SERVICE_ETP_ANNOTATION) {
            Some(e) => {
                if e.eq(&etp) {
                    None
                } else {
                    Some(&etp)
                }
            }
            None => Some(&etp),
        };
        if let Some(e) = new_etp {
            new_eps
                .annotations_mut()
                .insert(SERVICE_ETP_ANNOTATION.to_string(), e.to_string());
            need_update = true;
        }
        // sync allocation
        match new_eps
            .annotations_mut()
            .get_mut(SERVICE_ALLOCATION_ANNOTATION)
        {
            Some(val) => {
                if allocated_addr_str.ne(val.as_str()) {
                    *val = allocated_addr_str.to_string();
                    need_update = true;
                }
            }
            None => {
                new_eps.annotations_mut().insert(
                    SERVICE_ALLOCATION_ANNOTATION.to_string(),
                    allocated_addr_str.to_string(),
                );
                need_update = true;
            }
        }
        if need_update {
            endpointslice_api
                .replace(&eps.name_any(), &PostParams::default(), &new_eps)
                .await
                .map_err(Error::Kube)?;
        }
    }

    // Get from given service's stauts field.
    // This should be the single source of truth for current state.
    // Collected addresses must be marked as allocated.
    // Addresses that don't belong to any blocks will be ignored.
    // In this part, some blocks should be available.
    let actual_allocation: HashMap<String, Vec<MarkedAllocation>> = {
        let c = ctx.component.clone();
        let alloc_set = c.inner.lock().map_err(|_| Error::FailedToGetLock)?;
        alloc_set
            .get_blocks_from_addrs(&actual_addrs)
            .clone()
            .into_iter()
            .map(|(n, addrs)| {
                (
                    n,
                    addrs
                        .iter()
                        .map(|a| MarkedAllocation::Allocated(*a))
                        .collect::<Vec<MarkedAllocation>>(),
                )
            })
            .collect()
    };

    // Allocation information stored in allocators(in memory) may be flushed
    // because of restarting the controller.
    // Soe actual allocation that is from given Service resources is more reliable.

    // Desired allocation information is from annotations of given service resource.
    // This information should be stored in the controller and its resource('s status) finally.
    let desired_allocation = {
        let c = ctx.component.clone();
        build_desired_allocation(svc, &c)?
    };

    let merged_allocation = merge_marked_allocation(actual_allocation, desired_allocation);

    let c = ctx.component.clone();
    let mut remained = Vec::new();
    let mut removed = Vec::new();
    {
        let mut alloc_set = c.inner.lock().map_err(|_| Error::FailedToGetLock)?;
        for (block_name, allocs) in merged_allocation.iter() {
            if let Some(block) = alloc_set.get_mut(block_name) {
                let (mut a, mut r) = update_allocations(block, allocs)?;
                remained.append(&mut a);
                removed.append(&mut r);
            }
        }
    }
    if actual_addrs.eq(&remained) {
        return Ok(Action::await_change());
    }
    tracing::info!(name = svc.name_any(), namespace = ns, remained=?remained, released=?removed, "Update allocation.");

    let new_svc = update_svc_lb_addrs(svc, &remained);
    api.replace_status(
        &svc.name_any(),
        &PostParams::default(),
        serde_json::to_vec(&new_svc).map_err(Error::Serialization)?,
    )
    .await
    .map_err(Error::Kube)?;

    tracing::info!(
        name = svc.name_any(),
        namespace = ns,
        lb_addrs=?remained,
        "Update service status by the allocation lb address"
    );

    let new_allocated_addrs = get_allocated_lb_addrs(&new_svc)
        .map(|v| v.iter().map(|a| a.to_string()).collect::<Vec<String>>())
        .map(|v| v.join(","));
    match new_allocated_addrs {
        Some(addrs) => {
            for eps in epss.iter() {
                let mut new_eps = eps.clone();
                new_eps
                    .annotations_mut()
                    .insert(SERVICE_ALLOCATION_ANNOTATION.to_string(), addrs.clone());
                endpointslice_api
                    .replace(&eps.name_any(), &PostParams::default(), &new_eps)
                    .await
                    .map_err(Error::Kube)?;
            }
        }
        None => {
            for eps in epss.iter() {
                let mut new_eps = eps.clone();
                new_eps
                    .annotations_mut()
                    .remove(SERVICE_ALLOCATION_ANNOTATION);
                endpointslice_api
                    .replace(&eps.name_any(), &PostParams::default(), &new_eps)
                    .await
                    .map_err(Error::Kube)?;
            }
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(
    api: &Api<Service>,
    svc: &Service,
    ctx: Arc<ContextWith<Arc<AllocatorSet>>>,
) -> Result<Action, Error> {
    let ns = get_namespace::<Service>(svc).map_err(Error::KubeLibrary)?;
    tracing::info!(name = svc.name_any(), namespace = ns, "Cleanup Service");

    let allocated_addrs = match get_allocated_lb_addrs(svc) {
        Some(a) => a,
        None => return Ok(Action::await_change()),
    };

    // If lb addresses are allocated, release these addresses any way.
    let released = {
        let c = ctx.component.clone();
        release_lb_addrs(svc, &allocated_addrs, &c)?
    };

    let new_svc = clear_svc_lb_addrs(svc, &released);
    api.replace_status(
        &svc.name_any(),
        &PostParams::default(),
        serde_json::to_vec(&new_svc).map_err(Error::Serialization)?,
    )
    .await
    .map_err(Error::Kube)?;

    tracing::info!(
        name = svc.name_any(),
        namespace = ns,
        lb_addrs=?released,
        "Update service status by the release lb address"
    );

    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64, allocator_set: Arc<AllocatorSet>) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let services = Api::<Service>::all(client.clone());

    tracing::info!("Start Service watcher");

    Controller::new(services, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<Service, Error, ContextWith<Arc<AllocatorSet>>>,
            state.to_context_with(client, interval, allocator_set),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

pub fn is_loadbalancer(svc: &Service) -> bool {
    match svc.spec.as_ref().and_then(|spec| spec.type_.as_ref()) {
        Some(t) => t.eq("LoadBalancer"),
        None => false,
    }
}

pub fn get_allocated_lb_addrs(svc: &Service) -> Option<Vec<IpAddr>> {
    svc.status.clone().and_then(|svc_status| {
        svc_status.load_balancer.and_then(|lb_status| {
            lb_status.ingress.map(|ingress| {
                ingress
                    .iter()
                    .filter_map(|lb_ingress| {
                        lb_ingress.ip.as_ref().map(|ip| IpAddr::from_str(ip).ok())
                    })
                    .flatten()
                    .collect::<Vec<IpAddr>>()
            })
        })
    })
}

fn get_releasable_addrs(svc: &Service) -> Option<Vec<IpAddr>> {
    svc.annotations()
        .get(RELEASE_ANNOTATION)
        .map(|s| s.split(',').collect::<Vec<&str>>())
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| IpAddr::from_str(addr).ok())
                .collect::<Vec<IpAddr>>()
        })
        .filter(|addrs| !addrs.is_empty())
}

fn get_required_lb_addrs(svc: &Service) -> Option<Vec<IpAddr>> {
    svc.annotations()
        .get(LOADBALANCER_ADDRESS_ANNOTATION)
        .map(|s| s.split(',').collect::<Vec<&str>>())
        .map(|addrs| {
            addrs
                .iter()
                .filter_map(|addr| IpAddr::from_str(addr).ok())
                .collect::<Vec<IpAddr>>()
        })
        .filter(|addrs| !addrs.is_empty())
}

fn get_desired_pools(svc: &Service) -> Option<Vec<&str>> {
    svc.annotations()
        .get(ADDRESS_POOL_ANNOTATION)
        .map(|s| s.split(',').collect::<Vec<&str>>())
}

fn get_etp(svc: &Service) -> Option<String> {
    svc.spec
        .as_ref()
        .and_then(|spec| spec.external_traffic_policy.clone())
}

fn build_desired_allocation(
    svc: &Service,
    allocator: &Arc<AllocatorSet>,
) -> Result<HashMap<String, Vec<MarkedAllocation>>, Error> {
    let alloc_set = allocator.inner.lock().map_err(|_| Error::FailedToGetLock)?;
    let pools = match get_desired_pools(svc) {
        Some(pools) => pools.iter().map(|a| a.to_string()).collect::<Vec<String>>(),
        None => match &alloc_set.auto_assign {
            Some(a) => vec![a.to_string()],
            None => {
                return Err(Error::NoAllocatableAddress);
            }
        },
    };
    let res: HashMap<String, Vec<MarkedAllocation>> = match get_required_lb_addrs(svc) {
        Some(addrs) => {
            //
            let blocks = alloc_set.get_blocks_from_addrs(&addrs);
            pools
                .into_iter()
                .map(|name| match blocks.get(&name) {
                    Some(addrs) => {
                        let markers = addrs
                            .iter()
                            .map(|a| MarkedAllocation::NotMarked(Some(*a)))
                            .collect::<Vec<MarkedAllocation>>();
                        (name, markers)
                    }
                    None => (name, vec![MarkedAllocation::NotMarked(None)]),
                })
                .collect()
        }
        None => {
            // Desired addresses are not set.
            pools
                .into_iter()
                .map(|n| (n, vec![MarkedAllocation::NotMarked(None)]))
                .collect()
        }
    };

    Ok(res)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MarkedAllocation {
    NotMarked(Option<IpAddr>),
    Allocated(IpAddr),
    Allocate(Option<IpAddr>),
    Release(IpAddr),
}

fn update_allocations(
    block: &mut Block,
    allocs: &[MarkedAllocation],
) -> Result<(Vec<IpAddr>, Vec<IpAddr>), Error> {
    let mut remained = Vec::new();
    let mut removed = Vec::new();
    for ma in allocs.iter() {
        match ma {
            MarkedAllocation::Allocate(addr_opt) => match addr_opt {
                Some(addr) => {
                    // Allocate the specified address.
                    let addr = block.allocator.allocate(addr, false).map_err(Error::Ipam)?;
                    remained.push(addr);
                }
                None => {
                    // Allocate an adderss.
                    let addr = block.allocator.allocate_next().map_err(Error::Ipam)?;
                    remained.push(addr);
                }
            },
            MarkedAllocation::Release(addr) => {
                // Release the address.
                let addr = block.allocator.release(addr).map_err(Error::Ipam)?;
                removed.push(addr);
            }
            MarkedAllocation::Allocated(addr) => {
                // Allocate if not stored in the block.
                if !block.allocator.is_allocated(addr) {
                    block.allocator.allocate(addr, true).map_err(Error::Ipam)?;
                }
                remained.push(*addr);
            }
            MarkedAllocation::NotMarked(_a) => return Err(Error::UnavailableAllocation),
        }
    }
    remained.sort();
    removed.sort();
    Ok((remained, removed))
}

#[tracing::instrument(skip_all)]
fn release_lb_addrs(
    svc: &Service,
    allocated_addrs: &[IpAddr],
    allocator: &Arc<AllocatorSet>,
) -> Result<Vec<IpAddr>, Error> {
    let ns = get_namespace::<Service>(svc).map_err(Error::KubeLibrary)?;

    let mut release_result: Vec<IpAddr> = Vec::new();

    let mut alloc_set = allocator.inner.lock().map_err(|_| Error::FailedToGetLock)?;

    let pools = get_desired_pools(svc)
        .map(|p| p.iter().map(|p| p.to_string()).collect::<Vec<String>>())
        .unwrap_or(match &alloc_set.auto_assign {
            Some(d) => vec![d.to_string()],
            None => vec![],
        });

    if pools.is_empty() {
        return Err(Error::NoAllocatableBlock);
    }

    for pool in pools.iter() {
        let block = match alloc_set.blocks.get_mut(pool) {
            Some(b) => b,
            None => {
                tracing::warn!(
                    name = svc.name_any(),
                    namespace = ns,
                    desired_pool = pool,
                    "Desired AddressBlock doesn't exist"
                );
                continue;
            }
        };
        for addr in allocated_addrs.iter() {
            if !block.allocator.cidr().contains(addr) || !block.allocator.is_allocated(addr) {
                continue;
            }
            match block.allocator.release(addr) {
                Ok(a) => {
                    release_result.push(a);
                    tracing::info!(
                        name = svc.name_any(),
                        namespace = ns,
                        pool = block.pool_name,
                        address = a.to_string(),
                        "Relaese address from address pool"
                    )
                }
                Err(e) => {
                    tracing::error!(
                        error=?e,
                        name=svc.name_any(),
                        namespace = ns,
                        pool = block.pool_name,
                        "failed to allocate from requested address pool",
                    );
                }
            }
        }
    }

    Ok(release_result)
}

fn update_svc_lb_addrs(svc: &Service, addrs: &[IpAddr]) -> Service {
    let ingress: Vec<LoadBalancerIngress> = addrs
        .iter()
        .map(|a| LoadBalancerIngress {
            hostname: None,
            ip: Some(a.to_string()),
            ports: None,
        })
        .collect();
    let mut new_svc = svc.clone();
    match new_svc.status.as_mut() {
        Some(status) => match status.load_balancer.as_mut() {
            Some(lb_status) => {
                *lb_status = LoadBalancerStatus {
                    ingress: Some(ingress),
                };
            }
            None => {
                *status = ServiceStatus {
                    conditions: status.conditions.clone(),
                    load_balancer: Some(LoadBalancerStatus {
                        ingress: Some(ingress),
                    }),
                };
            }
        },
        None => {
            new_svc.status = Some(ServiceStatus {
                conditions: None, // TODO: fill conditions
                load_balancer: Some(LoadBalancerStatus {
                    ingress: Some(ingress),
                }),
            })
        }
    };

    new_svc
}

fn clear_svc_lb_addrs(svc: &Service, released: &[IpAddr]) -> Service {
    let mut new_svc = svc.clone();
    let released_str = released
        .iter()
        .map(|a| a.to_string())
        .collect::<Vec<String>>();

    if let Some(status) = new_svc.status.as_mut() {
        if let Some(lb_status) = status.load_balancer.as_mut() {
            if let Some(ingress) = lb_status.ingress.as_mut() {
                let mut new_ingress = Vec::new();
                for ir in ingress.iter() {
                    if let Some(ip) = &ir.ip {
                        if released_str.contains(ip) {
                            continue;
                        }
                    }
                    new_ingress.push(ir.clone());
                }
                *ingress = new_ingress;
            }
        }
    }
    new_svc
}

// fn get_diff(prev: &[String], now: &[String]) -> (Vec<String>, Vec<String>, Vec<String>) {
//     let removed = prev
//         .iter()
//         .filter(|p| !now.contains(p))
//         .cloned()
//         .collect::<Vec<String>>();
//     let added = now
//         .iter()
//         .filter(|n| !prev.contains(n) && !removed.contains(n))
//         .cloned()
//         .collect::<Vec<String>>();
//     let shared = prev
//         .iter()
//         .filter(|p| now.contains(p))
//         .cloned()
//         .collect::<Vec<String>>();
//     (added, shared, removed)
// }

fn merge_marked_allocation(
    actual_allocation: HashMap<String, Vec<MarkedAllocation>>,
    desired_allocation: HashMap<String, Vec<MarkedAllocation>>,
) -> HashMap<String, Vec<MarkedAllocation>> {
    let (added, shared, removed) = diff::<String>(
        &actual_allocation.keys().cloned().collect::<Vec<String>>(),
        &desired_allocation.keys().cloned().collect::<Vec<String>>(),
    );
    // Merge actual and stored allocation.
    let mut merged_allocation: HashMap<String, Vec<MarkedAllocation>> = HashMap::new();
    for added_name in added.iter() {
        if let Some(d_allocs) = desired_allocation.get(added_name) {
            let mut da = d_allocs
                .iter()
                .map(|ma| match ma {
                    MarkedAllocation::NotMarked(a) => MarkedAllocation::Allocate(*a),
                    _ => *ma,
                })
                .collect::<Vec<MarkedAllocation>>();
            match merged_allocation.get_mut(added_name) {
                Some(v) => v.append(&mut da),
                None => {
                    merged_allocation.insert(added_name.to_string(), da);
                }
            }
        }
    }
    for remvoed_name in removed.iter() {
        if let Some(a_allocs) = actual_allocation.get(remvoed_name) {
            let mut ra = a_allocs
                .iter()
                .filter_map(|ma| match ma {
                    MarkedAllocation::Allocated(a) => Some(MarkedAllocation::Release(*a)),
                    _ => None,
                })
                .collect::<Vec<MarkedAllocation>>();
            match merged_allocation.get_mut(remvoed_name) {
                Some(v) => v.append(&mut ra),
                None => {
                    merged_allocation.insert(remvoed_name.to_string(), ra);
                }
            }
        }
    }
    for shared_name in shared.iter() {
        // Shared
        // block named shared_name must exist. Checked existance by getting the diff.
        // So we can unwrap.
        let d_allocs = desired_allocation.get(shared_name).unwrap();
        let a_allocs = actual_allocation.get(shared_name).unwrap();
        if d_allocs.len() == 1 && d_allocs[0].eq(&MarkedAllocation::NotMarked(None)) {
            // allocation from a block(named d_name) is desired but not specified some addresses.
            // In this case, we use exisiting allocation as is.
            merged_allocation.insert(shared_name.to_string(), a_allocs.clone());
            continue;
        }
        // Some specific addresses are desired.
        let mut v: Vec<MarkedAllocation> = Vec::new();
        // Check if a desired address already allocated.
        // And if not, mark it as allocate.
        for da in d_allocs.iter() {
            if let MarkedAllocation::NotMarked(Some(addr)) = da {
                if a_allocs.contains(&MarkedAllocation::Allocated(*addr)) {
                    // Check if a desired address is already allocated.
                    v.push(MarkedAllocation::Allocated(*addr));
                } else {
                    // If not, newly allocate the specified address.
                    v.push(MarkedAllocation::Allocate(Some(*addr)));
                }
            }
        }
        // Check if an actual allocated address are not desired.
        for aa in a_allocs.iter() {
            if let MarkedAllocation::Allocated(addr) = aa {
                if !d_allocs.contains(&MarkedAllocation::NotMarked(Some(*addr))) {
                    v.push(MarkedAllocation::Release(*addr));
                }
            }
        }
        merged_allocation.insert(shared_name.to_string(), v);
    }
    merged_allocation
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
    };

    use ipnet::IpNet;
    use k8s_openapi::api::core::v1::{
        LoadBalancerIngress, LoadBalancerStatus, Service, ServiceStatus,
    };
    use kube::core::ObjectMeta;

    use super::*;
    use rstest::rstest;

    #[rstest(
		svc,
		expected,
		case(
        	Service {
        	    metadata: ObjectMeta {
        	        annotations: Some(BTreeMap::from([(
        	            LOADBALANCER_ADDRESS_ANNOTATION.to_string(),
        	            "10.0.0.1".to_string(),
        	        )])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
			Some(vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))])
		),
		case(
        	Service {
        	    metadata: ObjectMeta {
					annotations: None,
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
			None,
		),
		case(
        	Service {
        	    metadata: ObjectMeta {
        	        annotations: Some(BTreeMap::from([(
        	            LOADBALANCER_ADDRESS_ANNOTATION.to_string(),
        	            "10.0.0.1,10.0.0.3".to_string(),
        	        )])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
			Some(vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3))])
		),
		case(
        	Service {
        	    metadata: ObjectMeta {
        	        annotations: Some(BTreeMap::from([(
        	            LOADBALANCER_ADDRESS_ANNOTATION.to_string(),
        	            "2001:db8::1".to_string(),
        	        )])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
			Some(vec![IpAddr::V6(Ipv6Addr::from_str("2001:db8::1").unwrap())])
		),
		case(
        	Service {
        	    metadata: ObjectMeta {
        	        annotations: Some(BTreeMap::from([(
        	            LOADBALANCER_ADDRESS_ANNOTATION.to_string(),
        	            "2001:db8::1,10.0.0.1,10.0.0.3".to_string(),
        	        )])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
			Some(vec![IpAddr::V6(Ipv6Addr::from_str("2001:db8::1").unwrap()), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3))])
		),
	)]
    fn test_get_required_lb_addrs(svc: Service, expected: Option<Vec<IpAddr>>) {
        let res = get_required_lb_addrs(&svc);
        assert_eq!(res, expected);
    }
    #[rstest(
		svc,
		expected,
		case(
        	Service {
        	    metadata: ObjectMeta {
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: Some(ServiceStatus {
        	        conditions: None,
        	        load_balancer: Some(LoadBalancerStatus {
        	            ingress: Some(vec![LoadBalancerIngress {
        	                ip: Some("10.0.0.1".to_string()),
        	                ..Default::default()
        	            }]),
        	        }),
        	    }),
        	},
			Some(vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))])
		),
		case(
        	Service {
        	    metadata: ObjectMeta {
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
			None,
		),
		case(
        	Service {
        	    metadata: ObjectMeta {
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: Some(ServiceStatus {
        	        conditions: None,
        	        load_balancer: Some(LoadBalancerStatus {
        	            ingress: Some(vec![
							LoadBalancerIngress {
        	                	ip: Some("10.0.0.1".to_string()),
        	                	..Default::default()
        	            	},
							LoadBalancerIngress {
        	                	ip: Some("10.0.0.3".to_string()),
        	                	..Default::default()
        	            	},
						]),
        	        }),
        	    }),
        	},
			Some(vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3))])
		),
		case(
        	Service {
        	    metadata: ObjectMeta {
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: Some(ServiceStatus {
        	        conditions: None,
        	        load_balancer: Some(LoadBalancerStatus {
        	            ingress: Some(vec![LoadBalancerIngress {
        	                ip: Some("2001:db8::1".to_string()),
        	                ..Default::default()
        	            }]),
        	        }),
        	    }),
        	},
			Some(vec![IpAddr::V6(Ipv6Addr::from_str("2001:db8::1").unwrap())])
		),
		case(
        	Service {
        	    metadata: ObjectMeta {
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: Some(ServiceStatus {
        	        conditions: None,
        	        load_balancer: Some(LoadBalancerStatus {
        	            ingress: Some(vec![
							LoadBalancerIngress {
        	                	ip: Some("2001:db8::1".to_string()),
        	                	..Default::default()
        	            	},
							LoadBalancerIngress {
        	                	ip: Some("10.0.0.1".to_string()),
        	                	..Default::default()
        	            	},
							LoadBalancerIngress {
        	                	ip: Some("10.0.0.3".to_string()),
        	                	..Default::default()
        	            	},
						]),
        	        }),
        	    }),
        	},
			Some(vec![IpAddr::V6(Ipv6Addr::from_str("2001:db8::1").unwrap()), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)), IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3))])
		),
	)]
    fn test_get_allocated_lb_addrs(svc: Service, expected: Option<Vec<IpAddr>>) {
        let res = get_allocated_lb_addrs(&svc);
        assert_eq!(res, expected);
    }

    #[rstest(
        prev,
        now,
        expected,
        case(
            vec!["a".to_string(), "b".to_string()],
            vec!["a".to_string(), "b".to_string()],
            (vec![], vec!["a".to_string(), "b".to_string()], vec![]),
        ),
        case(
            vec!["a".to_string()],
            vec!["a".to_string(), "b".to_string()],
            (vec!["b".to_string()], vec!["a".to_string()], vec![]),
        ),
        case(
            vec!["a".to_string(), "b".to_string()],
            vec!["b".to_string()],
            (vec![], vec!["b".to_string()], vec!["a".to_string()]),
        ),
        case(
            vec!["a".to_string(), "c".to_string(), "d".to_string()],
            vec!["b".to_string(), "c".to_string()],
            (vec!["b".to_string()], vec!["c".to_string()], vec!["a".to_string(), "d".to_string()]),
        ),
    )]
    fn works_diff(
        prev: Vec<String>,
        now: Vec<String>,
        expected: (Vec<String>, Vec<String>, Vec<String>),
    ) {
        let (added, shared, removed) = diff::<String>(&prev, &now);
        assert_eq!(added, expected.0);
        assert_eq!(shared, expected.1);
        assert_eq!(removed, expected.2);
    }

    #[rstest(
        svc,
        allocator,
        expected,
        case(
        	Service {
        	    metadata: ObjectMeta {
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
            test_allocator_set(),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(None)])]),
        ),
        case(
        	Service {
        	    metadata: ObjectMeta {
                    annotations: Some(BTreeMap::from([(LOADBALANCER_ADDRESS_ANNOTATION.to_string(), "10.0.0.2".to_string())])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
            test_allocator_set(),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(Some(IpAddr::from_str("10.0.0.2").unwrap()))])]),
        ),
        case(
        	Service {
        	    metadata: ObjectMeta {
                    annotations: Some(BTreeMap::from([(LOADBALANCER_ADDRESS_ANNOTATION.to_string(), "10.0.0.2,10.0.0.10".to_string())])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
            test_allocator_set(),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(Some(IpAddr::from_str("10.0.0.2").unwrap())), MarkedAllocation::NotMarked(Some(IpAddr::from_str("10.0.0.10").unwrap()))])]),
        ),
        case(
        	Service {
        	    metadata: ObjectMeta {
                    annotations: Some(BTreeMap::from([(ADDRESS_POOL_ANNOTATION.to_string(), "test-pool-another".to_string())])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
            test_allocator_set(),
            HashMap::from([("test-pool-another".to_string(), vec![MarkedAllocation::NotMarked(None)])]),
        ),
        case(
        	Service {
        	    metadata: ObjectMeta {
                    annotations: Some(BTreeMap::from([(ADDRESS_POOL_ANNOTATION.to_string(), "test-pool-another".to_string()), (LOADBALANCER_ADDRESS_ANNOTATION.to_string(), "10.0.1.5".to_string())])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
            test_allocator_set(),
            HashMap::from([("test-pool-another".to_string(), vec![MarkedAllocation::NotMarked(Some(IpAddr::from_str("10.0.1.5").unwrap()))])]),
        ),
        case(
        	Service {
        	    metadata: ObjectMeta {
                    annotations: Some(BTreeMap::from([(ADDRESS_POOL_ANNOTATION.to_string(), "test-pool".to_string())])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
            test_allocator_set(),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(None)])]),
        ),
        case(
        	Service {
        	    metadata: ObjectMeta {
                    annotations: Some(BTreeMap::from([(ADDRESS_POOL_ANNOTATION.to_string(), "test-pool,test-pool-another".to_string()), (LOADBALANCER_ADDRESS_ANNOTATION.to_string(), "10.0.0.111,10.0.1.6".to_string())])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
            test_allocator_set(),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(Some(IpAddr::from_str("10.0.0.111").unwrap()))]), ("test-pool-another".to_string(), vec![MarkedAllocation::NotMarked(Some(IpAddr::from_str("10.0.1.6").unwrap()))])]),
        ),
        case(
        	Service {
        	    metadata: ObjectMeta {
                    annotations: Some(BTreeMap::from([(ADDRESS_POOL_ANNOTATION.to_string(), "test-pool,test-pool-another".to_string()), (LOADBALANCER_ADDRESS_ANNOTATION.to_string(), "10.0.1.7".to_string())])),
        	        ..Default::default()
        	    },
        	    spec: None,
        	    status: None,
        	},
            test_allocator_set(),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(None)]), ("test-pool-another".to_string(), vec![MarkedAllocation::NotMarked(Some(IpAddr::from_str("10.0.1.7").unwrap()))])]),
        ),
    )]
    fn works_build_desired_allocation(
        svc: Service,
        allocator: Arc<AllocatorSet>,
        expected: HashMap<String, Vec<MarkedAllocation>>,
    ) {
        let res = build_desired_allocation(&svc, &allocator).unwrap();
        assert_eq!(expected, res);
    }

    #[rstest(
        desired,
        actual,
        expected,
        case(HashMap::new(), HashMap::new(), HashMap::new()),
        case(
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(None)])]),
            HashMap::new(),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::Allocate(None)])]),
        ),
        case(
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(Some(IpAddr::from_str("10.0.0.100").unwrap()))])]),
            HashMap::new(),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::Allocate(Some(IpAddr::from_str("10.0.0.100").unwrap()))])]),
        ),
        case(
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(None)])]),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::Allocated(IpAddr::from_str("10.0.0.0").unwrap())])]),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::Allocated(IpAddr::from_str("10.0.0.0").unwrap())])]),
        ),
        case(
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(None)]), ("test-pool-another".to_string(), vec![MarkedAllocation::NotMarked(None)])]),
            HashMap::new(),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::Allocate(None)]), ("test-pool-another".to_string(), vec![MarkedAllocation::Allocate(None)])]),
        ),
        case(
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(None)]), ("test-pool-another".to_string(), vec![MarkedAllocation::NotMarked(None)])]),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::Allocated(IpAddr::from_str("10.0.0.0").unwrap())])]),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::Allocated(IpAddr::from_str("10.0.0.0").unwrap())]), ("test-pool-another".to_string(), vec![MarkedAllocation::Allocate(None)])]),
        ),
        case(
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(None)]), ("test-pool-another".to_string(), vec![MarkedAllocation::NotMarked(Some(IpAddr::from_str("10.0.1.7").unwrap()))])]),
            HashMap::new(),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::Allocate(None)]), ("test-pool-another".to_string(), vec![MarkedAllocation::Allocate(Some(IpAddr::from_str("10.0.1.7").unwrap()))])]),
        ),
        case(
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::NotMarked(Some(IpAddr::from_str("10.0.0.100").unwrap()))])]),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::Allocated(IpAddr::from_str("10.0.0.1").unwrap())])]),
            HashMap::from([("test-pool".to_string(), vec![MarkedAllocation::Allocate(Some(IpAddr::from_str("10.0.0.100").unwrap())), MarkedAllocation::Release(IpAddr::from_str("10.0.0.1").unwrap())])]),
        ),
    )]
    fn works_merge_marked_allocation(
        desired: HashMap<String, Vec<MarkedAllocation>>,
        actual: HashMap<String, Vec<MarkedAllocation>>,
        expected: HashMap<String, Vec<MarkedAllocation>>,
    ) {
        let res = merge_marked_allocation(actual, desired);
        assert_eq!(expected, res);
    }

    fn test_allocator_set() -> Arc<AllocatorSet> {
        let a = Arc::new(AllocatorSet::new());
        {
            let mut ai = a.inner.lock().unwrap();
            let b1 = Block::new(
                "test-pool".to_string(),
                "test-pool".to_string(),
                IpNet::from_str("10.0.0.0/24").unwrap(),
            )
            .unwrap();
            let b2 = Block::new(
                "test-pool-another".to_string(),
                "test-pool-another".to_string(),
                IpNet::from_str("10.0.1.0/24").unwrap(),
            )
            .unwrap();
            ai.insert(b1, true).unwrap();
            ai.insert(b2, false).unwrap();
        }
        a
    }
}
