use std::{net::IpAddr, str::FromStr, sync::Arc};

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
    controller::error::Error,
    ipam::manager::{AllocatorSet, Block},
    kubernetes::{
        context::{error_policy, ContextWith, Ctx, State},
        crd::address_pool::{ADDRESS_POOL_ANNOTATION, LOADBALANCER_ADDRESS_ANNOTATION},
        util::get_namespace,
    },
};

const SERVICE_FINALIZER: &str = "service.sart.terassyi.net/finalizer";
pub(super) const SERVICE_NAME_LABEL: &str = "kubernetes.io/service-name";
// This annoation should be annotated to Endpointslices to handle externalTrafficPolicy
// This annotation is used only for triggering the Endpoinslice's reconciliation loop
const SERVICE_ETP_ANNOTATION: &str = "service.sart.terassyi.net/etp";

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(
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

    let allocated_addrs = get_allocated_lb_addrs(svc);

    if !is_loadbalacner(svc) {
        // If Service is not LoadBalancer and it hash allocated load balancer addresses, release these
        if let Some(allocated) = allocated_addrs {
            let released = {
                let c = ctx.component.clone();
                release_lb_addrs(svc, allocated, &c)?
            };
            let new_svc = clear_svc_lb_addrs(svc, &released);
            api.replace_status(
                &svc.name_any(),
                &PostParams::default(),
                serde_json::to_vec(&new_svc).map_err(Error::Serialization)?,
            )
            .await
            .map_err(Error::Kube)?;

            tracing::warn!(
                name = svc.name_any(),
                namespace = ns,
                lb_addrs=?released,
                "Release lb addresses because this service is not a load balancer");
        }
        return Ok(Action::await_change());
    }

    // In case of Service is LoadBalancer type

    // Sync the externalTrafficPolicy value
    let etp = get_etp(svc).ok_or(Error::InvalidExternalTrafficPolicy)?;

    let endpointslice_api = Api::<EndpointSlice>::namespaced(ctx.client().clone(), &ns);
    let lp = ListParams::default().labels(&format!("{}={}", SERVICE_NAME_LABEL, svc.name_any()));
    let epss = endpointslice_api.list(&lp).await.map_err(Error::Kube)?;

    for eps in epss.items.iter() {
        let new_etp = match eps.annotations().get(SERVICE_ETP_ANNOTATION) {
            Some(e) => {
                if e.eq(&etp) {
                    None
                } else {
                    Some(e)
                }
            }
            None => Some(&etp),
        };
        if let Some(e) = new_etp {
            let mut new_eps = eps.clone();
            new_eps
                .annotations_mut()
                .insert(SERVICE_ETP_ANNOTATION.to_string(), e.to_string());

            endpointslice_api
                .replace(&eps.name_any(), &PostParams::default(), &new_eps)
                .await
                .map_err(Error::Kube)?;
        }
    }

    if let Some(allocated) = allocated_addrs {
        // If lb addresses are already allocated,
        // validate these
        let _c = ctx.component.clone();

        tracing::info!(
            name = svc.name_any(),
            namespace = ns,
            lb_addrs=?allocated,
            "Already allocated"
        );

        return Ok(Action::await_change());
    }

    let new_allocation = {
        let c = ctx.component.clone();
        allocate_lb_addrs(svc, &c)?
    };

    let new_svc = update_svc_lb_addrs(svc, &new_allocation);
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
        lb_addrs=?new_allocation,
        "Update service status by the allocation lb address"
    );

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
        release_lb_addrs(svc, allocated_addrs, &c)?
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

pub(crate) async fn run(state: State, interval: u64, allocator_set: Arc<AllocatorSet>) {
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

pub(super) fn is_loadbalacner(svc: &Service) -> bool {
    match svc.spec.as_ref().and_then(|spec| spec.type_.as_ref()) {
        Some(t) => t.eq("LoadBalancer"),
        None => false,
    }
}

pub(super) fn get_allocated_lb_addrs(svc: &Service) -> Option<Vec<IpAddr>> {
    svc.status.clone().and_then(|svc_status| {
        svc_status.load_balancer.and_then(|lb_status| {
            lb_status.ingress.map(|ingress| {
                ingress
                    .iter()
                    .filter_map(|lb_ingress| {
                        lb_ingress.ip.as_ref().map(|ip| IpAddr::from_str(&ip).ok())
                    })
                    .flatten()
                    .collect::<Vec<IpAddr>>()
            })
        })
    })
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

#[tracing::instrument(skip_all)]
fn allocate_lb_addrs(svc: &Service, allocator: &Arc<AllocatorSet>) -> Result<Vec<IpAddr>, Error> {
    let ns = get_namespace::<Service>(svc).map_err(Error::KubeLibrary)?;

    let desired_pools = get_desired_pools(svc);

    let req_addr_opt = get_required_lb_addrs(svc);

    let mut alloc_set = allocator.inner.lock().map_err(|_| Error::FailedToGetLock)?;

    let mut allocation_result: Vec<IpAddr> = Vec::new();

    let allocate = |block: &mut Block| -> Result<Option<IpAddr>, Error> {
        match req_addr_opt {
            Some(ref reqs) => {
                for r in reqs.iter() {
                    if !block.allocator.cidr().contains(r) {
                        continue;
                    }
                    // allocate
                    return block.allocator.allocate(r).map(Some).map_err(Error::Ipam);
                }
                Ok(None)
            }
            None => {
                // allocate automatically
                block
                    .allocator
                    .allocate_next()
                    .map(Some)
                    .map_err(Error::Ipam)
            }
        }
    };

    match desired_pools {
        Some(pools) => {
            for pool in pools.iter() {
                let block = match alloc_set.blocks.get_mut(*pool) {
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

                match allocate(block) {
                    Ok(res) => match res {
                        Some(a) => {
                            allocation_result.push(a);
                            tracing::info!(
                                name = svc.name_any(),
                                namespace = ns,
                                pool = block.pool_name,
                                address = a.to_string(),
                                "Allocate requested from address pool"
                            )
                        }
                        None => { /* pool's cidr and requested address is not matched */ }
                    },
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
        None => {
            tracing::info!("Allocate from auto assignable pool");
            for pool in alloc_set.auto_assigns.clone().iter() {
                let block = match alloc_set.blocks.get_mut(pool) {
                    Some(b) => b,
                    None => {
                        tracing::warn!(
                            name = svc.name_any(),
                            namespace = ns,
                            desired_pool = pool,
                            "AddressBlock doesn't exist"
                        );
                        continue;
                    }
                };

                match allocate(block) {
                    Ok(res) => match res {
                        Some(a) => {
                            allocation_result.push(a);
                            tracing::info!(
                                name = svc.name_any(),
                                namespace = ns,
                                pool = block.pool_name,
                                address = a.to_string(),
                                "Allocate an address from auto asssignable address pool"
                            );
                            // If pool names are not specified, it is ok to allocate at least one address
                            break;
                        }
                        None => { /* pool's cidr and requested address is not matched */ }
                    },
                    Err(e) => {
                        tracing::error!(
                            error=?e,
                            name=svc.name_any(),
                            namespace = ns,
                            pool = block.pool_name,
                            "failed to allocate address",
                        );
                    }
                }
            }
        }
    }
    if allocation_result.is_empty() {
        Err(Error::NoAllocatableAddress)
    } else {
        Ok(allocation_result)
    }
}

fn release_lb_addrs(
    svc: &Service,
    allocated_addrs: Vec<IpAddr>,
    allocator: &Arc<AllocatorSet>,
) -> Result<Vec<IpAddr>, Error> {
    let ns = get_namespace::<Service>(svc).map_err(Error::KubeLibrary)?;

    let mut release_result: Vec<IpAddr> = Vec::new();

    let mut alloc_set = allocator.inner.lock().map_err(|_| Error::FailedToGetLock)?;

    let pools = get_desired_pools(svc)
        .map(|p| p.iter().map(|p| p.to_string()).collect::<Vec<String>>())
        .unwrap_or(alloc_set.auto_assigns.clone());

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
                // TODO: consider weather we can override lb status
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

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
    };

    use k8s_openapi::api::core::v1::{
        LoadBalancerIngress, LoadBalancerStatus, Service, ServiceStatus,
    };
    use kube::core::ObjectMeta;

    use crate::kubernetes::crd::address_pool::LOADBALANCER_ADDRESS_ANNOTATION;

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
}
