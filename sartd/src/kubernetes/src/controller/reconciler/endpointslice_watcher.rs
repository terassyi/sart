use std::{
    collections::{BTreeMap, HashMap},
    net::IpAddr,
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use futures::StreamExt;
use ipnet::IpNet;
use k8s_openapi::api::{
    core::v1::{LoadBalancerIngress, LoadBalancerStatus, Service, ServiceStatus},
    discovery::v1::EndpointSlice,
};
use kube::{
    api::{ListParams, PostParams},
    core::ObjectMeta,
    runtime::{
        controller::Action,
        finalizer::{finalizer, Event},
        watcher::Config,
        Controller,
    },
    Api, Client, ResourceExt,
};

use sartd_ipam::manager::AllocatorSet;

use crate::{
    context::{error_policy, Context, Ctx, State},
    controller::{
        error::Error,
        reconciler::service_watcher::{get_allocated_lb_addrs, is_loadbalacner},
    },
    crd::{
        address_pool::{AddressType, ADDRESS_POOL_ANNOTATION, LOADBALANCER_ADDRESS_ANNOTATION},
        bgp_advertisement::{
            AdvertiseStatus, BGPAdvertisement, BGPAdvertisementSpec, BGPAdvertisementStatus,
            Protocol,
        },
        bgp_peer::PEER_GROUP_ANNOTATION,
        node_bgp::NodeBGP,
    },
    util::{create_owner_reference, get_namespace},
};

use super::service_watcher::SERVICE_NAME_LABEL;

const ENDPOINTSLICE_FINALIZER: &str = "endpointsliece.sart.terassyi.net/finalizer";

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(eps: Arc<EndpointSlice>, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<EndpointSlice>(&eps).map_err(Error::KubeLibrary)?;

    let endpointslices = Api::<EndpointSlice>::namespaced(ctx.client().clone(), &ns);

    finalizer(
        &endpointslices,
        ENDPOINTSLICE_FINALIZER,
        eps,
        |event| async {
            match event {
                Event::Apply(eps) => reconcile(&eps, ctx.clone()).await,
                Event::Cleanup(eps) => cleanup(&eps, ctx.clone()).await,
            }
        },
    )
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(eps: &EndpointSlice, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<EndpointSlice>(eps).map_err(Error::KubeLibrary)?;
    tracing::info!(
        name = eps.name_any(),
        namespace = ns,
        "Reconcile Endpointslice"
    );

    let svc_name = match get_svc_name_from_eps(eps) {
        Some(n) => n,
        None => return Ok(Action::await_change()),
    };
    let services = Api::<Service>::namespaced(ctx.client().clone(), &ns);
    let svc = match services.get_opt(svc_name).await.map_err(Error::Kube)? {
        Some(svc) => svc,
        None => {
            tracing::warn!(
                name = svc_name,
                "The Service resource associated with EndpointSlice is not found"
            );
            return Ok(Action::await_change());
        }
    };

    if !is_loadbalacner(&svc) {
        return Ok(Action::await_change());
    }

    let lb_addrs = match get_allocated_lb_addrs(&svc) {
        Some(lb_addrs) => lb_addrs,
        None => {
            tracing::warn!(
                name = svc.name_any(),
                namespace = ns,
                "LoadBalancer address is not allocated yet. Reconcile after 10 seconds"
            );
            return Ok(Action::requeue(Duration::from_secs(10)));
        }
    };

    let node_bgps = Api::<NodeBGP>::all(ctx.client().clone());
    let target_peers = get_target_peers(eps, &svc, &node_bgps).await?;

    let bgp_advertisements = Api::<BGPAdvertisement>::namespaced(ctx.client().clone(), ns.as_str());

    // If Endpointslice has no endpoint, requeue and wait for creating endpoints
    // In case of creating new BGPAdvertisement, there are the case that endpointslice has no valid endpoint
    let mut need_requeue = target_peers.is_empty();
    for addr in lb_addrs.iter() {
        let cidr = IpNet::new(*addr, 32).map_err(|_| Error::InvalidAddress)?;
        let adv_name = format!("{}-{}", eps.name_any(), addr);

        match bgp_advertisements
            .get_opt(&adv_name)
            .await
            .map_err(Error::Kube)?
        {
            Some(adv) => {
                let mut new_adv = adv.clone();

                if adv.spec.cidr.ne(cidr.to_string().as_str()) {
                    tracing::warn!(
                        name = svc.name_any(),
                        namespace = ns,
                        "LoadBalancer address is changed"
                    );
                    new_adv.spec.cidr = cidr.to_string();
                    let mut new_target_peers: BTreeMap<String, AdvertiseStatus> = BTreeMap::new();
                    if let Some(target_peers) = new_adv.status.and_then(|target| target.peers) {
                        for (target, _status) in target_peers.iter() {
                            new_target_peers.insert(target.clone(), AdvertiseStatus::NotAdvertised);
                        }
                    }
                    new_adv.status = Some(BGPAdvertisementStatus {
                        peers: Some(new_target_peers),
                    });
                    bgp_advertisements
                        .replace_status(
                            &new_adv.name_any(),
                            &PostParams::default(),
                            serde_json::to_vec(&new_adv).map_err(Error::Serialization)?,
                        )
                        .await
                        .map_err(Error::Kube)?;
                }

                let need_update = match new_adv
                    .status
                    .as_mut()
                    .and_then(|status| status.peers.as_mut())
                {
                    Some(peers) => sync_target_peers(peers, &target_peers),
                    None => {
                        let mut peers: BTreeMap<String, AdvertiseStatus> = BTreeMap::new();
                        for target in target_peers.iter() {
                            peers.insert(target.clone(), AdvertiseStatus::NotAdvertised);
                        }
                        new_adv.status = Some(BGPAdvertisementStatus { peers: Some(peers) });
                        true
                    }
                };

                tracing::info!(name = adv.name_any(), namespace = ns, targets =? target_peers, status =? new_adv.status, "Sync target peers");

                if need_update {
                    bgp_advertisements
                        .replace_status(
                            &new_adv.name_any(),
                            &PostParams::default(),
                            serde_json::to_vec(&new_adv).map_err(Error::Serialization)?,
                        )
                        .await
                        .map_err(Error::Kube)?;
                }
            }
            None => {
                // create new Advertisement
                let mut peers = BTreeMap::new();
                for p in target_peers.iter() {
                    peers.insert(p.clone(), AdvertiseStatus::NotAdvertised);
                }

                let adv = BGPAdvertisement {
                    metadata: ObjectMeta {
                        name: Some(adv_name),
                        namespace: eps.namespace(),
                        owner_references: Some(vec![create_owner_reference(eps)]),
                        ..Default::default()
                    },
                    spec: BGPAdvertisementSpec {
                        cidr: cidr.to_string(),
                        r#type: AddressType::Service,
                        protocol: Protocol::from(&cidr),
                        attrs: None, // TODO: implement BGP attributes
                    },
                    status: Some(BGPAdvertisementStatus { peers: Some(peers) }),
                };

                tracing::info!(
                    name = adv.name_any(),
                    namespace = ns,
                    status = ?adv.status,
                    "Create BGPAdvertisement"
                );

                bgp_advertisements
                    .create(&PostParams::default(), &adv)
                    .await
                    .map_err(Error::Kube)?;

                need_requeue = true;
            }
        }
    }
    if need_requeue {
        Ok(Action::requeue(Duration::from_secs(10)))
    } else {
        Ok(Action::await_change())
    }
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(eps: &EndpointSlice, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<EndpointSlice>(eps).map_err(Error::KubeLibrary)?;

    tracing::info!(
        name = eps.name_any(),
        namespace = ns,
        "Cleanup Endpointslice"
    );

    // let svc_name = match get_svc_name_from_eps(eps) {
    //     Some(n) => n,
    //     None => return Ok(Action::await_change()),
    // };
    // let services = Api::<Service>::namespaced(ctx.client().clone(), &ns);
    // let svc = match services.get_opt(svc_name).await.map_err(Error::Kube)? {
    //     Some(svc) => svc,
    //     None => {
    //         tracing::warn!(
    //             name = svc_name,
    //             "The Service resource associated with EndpointSlice is not found"
    //         );
    //         return Ok(Action::await_change());
    //     }
    // };

    // if !is_loadbalacner(&svc) {
    //     return Ok(Action::await_change());
    // }

    // {
    //     let component = ctx.component.clone();
    //     release_lb_addr(&component, &svc)?;
    // }

    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let endpointslices = Api::<EndpointSlice>::all(client.clone());

    tracing::info!("Start Endpointslice watcher");

    Controller::new(endpointslices, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<EndpointSlice, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

#[tracing::instrument(skip_all)]
fn allocate_lb_addr(allocator: &Arc<AllocatorSet>, svc: &Service) -> Result<IpAddr, Error> {
    let mut alloc_set = allocator.inner.lock().map_err(|_| Error::FailedToGetLock)?;

    // Get address pool names from specified annoation.
    // If valid annotations are not specified, get pools that is set auto-assign as true from AllocatorSet from given context.
    let pools = svc
        .annotations()
        .get(ADDRESS_POOL_ANNOTATION)
        .map(|p| vec![p.clone()])
        .unwrap_or(alloc_set.auto_assigns.clone());

    // TODO: handle multiple addresses
    let lb_ip = match svc.annotations().get(LOADBALANCER_ADDRESS_ANNOTATION) {
        Some(addr) => match IpAddr::from_str(addr) {
            Ok(ip) => Some(ip),
            Err(e) => {
                tracing::warn!("failed to parse given loadBalancerIPs");
                None
            }
        },
        None => None,
    };

    for pool_name in pools.iter() {
        let block = match alloc_set.blocks.get_mut(pool_name) {
            Some(b) => b,
            None => continue,
        };

        if let Some(lb_ip) = lb_ip {
            if !block.allocator.cidr().contains(&lb_ip) {
                continue;
            }
            // try to allocate the specified address
            return block.allocator.allocate(&lb_ip).map_err(Error::Ipam);
        } else {
            match block.allocator.allocate_next() {
                Ok(addr) => return Ok(addr),
                Err(e) => match e {
                    sartd_ipam::error::Error::Full => {
                        tracing::warn!(name = block.name, "address block is full");
                        continue;
                    }
                    _ => return Err(Error::Ipam(e)),
                },
            }
        }
    }

    // if reach here, address is not allocated.
    Err(Error::NoAllocatableAddress)
}

#[tracing::instrument(skip_all)]
fn release_lb_addr(allocator: &Arc<AllocatorSet>, svc: &Service) -> Result<Option<IpAddr>, Error> {
    let ns = get_namespace::<Service>(svc).map_err(Error::KubeLibrary)?;

    let mut alloc_set = allocator.inner.lock().map_err(|_| Error::FailedToGetLock)?;

    // Get address pool names from specified annoation.
    // If valid annotations are not specified, get pools that is set auto-assign as true from AllocatorSet from given context.
    let pools = svc
        .annotations()
        .get(ADDRESS_POOL_ANNOTATION)
        .map(|p| vec![p.clone()])
        .unwrap_or(alloc_set.auto_assigns.clone());

    // TODO: handle multiple addresses
    let lb_addrs = match svc.status.clone().and_then(|lb| {
        lb.load_balancer.and_then(|lb_status| {
            lb_status.ingress.map(|ingresses| {
                ingresses
                    .iter()
                    .filter_map(|ingress| ingress.ip.clone())
                    .filter_map(|ip| IpAddr::from_str(&ip).ok())
                    .collect::<Vec<IpAddr>>()
            })
        })
    }) {
        Some(a) => a,
        None => {
            tracing::warn!(
                name = svc.name_any(),
                namespace = ns,
                "Lb address is not allocated for the service to clean up"
            );
            return Ok(None);
        }
    };

    tracing::info!(
        name = svc.name_any(),
        namespace = ns,
        "Release lb addresses"
    );
    let mut is_error = false;
    for pool_name in pools.iter() {
        let block = match alloc_set.blocks.get_mut(pool_name) {
            Some(b) => b,
            None => continue,
        };

        // lb_addrs should be length 1
        for addr in lb_addrs.iter() {
            if !block.allocator.cidr().contains(addr) {
                continue;
            }
            if let Err(e) = block.allocator.release(addr).map_err(Error::Ipam) {
                tracing::error!(error=?e,name=svc.name_any(),namespace=ns,address=addr.to_string(),block=block.name,"Failed to release lb address");
                is_error = true;
            }
        }
    }

    if is_error {
        Err(Error::ReleaseAddress)
    } else {
        Ok(if lb_addrs.is_empty() {
            None
        } else {
            Some(lb_addrs[0])
        })
    }
}

fn update_svc_lb_addresses(svc: &Service, addrs: &[IpAddr]) -> Service {
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

#[tracing::instrument(skip_all)]
async fn get_target_peers(
    eps: &EndpointSlice,
    svc: &Service,
    nb_api: &Api<NodeBGP>,
) -> Result<Vec<String>, Error> {
    let ns = match eps.namespace() {
        Some(ns) => ns,
        None => {
            tracing::warn!(
                name = eps.name_any(),
                "Namespace is not set in EndpointSlice"
            );
            return Err(Error::FailedToGetData("Namespace is required".to_string()));
        }
    };
    let mut target_peers = Vec::new();

    if let Some(svc_spec) = &svc.spec {
        if let Some(etp) = &svc_spec.external_traffic_policy {
            match etp.as_str() {
                "Cluster" => {
                    let nb_lists = nb_api
                        .list(&ListParams::default())
                        .await
                        .map_err(Error::Kube)?;
                    for nb in nb_lists.items.iter() {
                        let groups = get_svc_peer_groups(svc);
                        if let Some(peers) = &nb.spec.peers {
                            for peer in peers.iter() {
                                if peer.match_groups(&groups) {
                                    target_peers.push(peer.name.clone())
                                }
                            }
                        }
                    }
                }
                "Local" => {
                    for ep in eps.endpoints.iter() {
                        // Check wether its endpoint is available(serving?? or ready??)
                        // The lifecycle of kubernetes endpoints
                        // ref: https://kubernetes.io/docs/tutorials/services/pods-and-endpoint-termination-flow/
                        let cond = match ep.conditions.as_ref() {
                            Some(cond) => cond,
                            None => continue,
                        };
                        if !cond.serving.unwrap_or(false) {
                            tracing::warn!(name = svc.name_any(), namespace = ns, endpoints=?ep.addresses, "Endpoint is not serving");
                            continue;
                        }
                        if let Some(node_name) = &ep.node_name {
                            match nb_api.get_opt(node_name).await.map_err(Error::Kube)? {
                                Some(nb) => {
                                    // Compare labels the Service has and labels each NodeBGP has
                                    //
                                    let groups = get_svc_peer_groups(svc);
                                    if let Some(peers) = nb.spec.peers {
                                        for peer in peers.iter() {
                                            if peer.match_groups(&groups) {
                                                target_peers.push(peer.name.clone())
                                            }
                                        }
                                    }
                                }
                                None => tracing::warn!(
                                    name = svc.name_any(),
                                    namespace = ns,
                                    node = node_name,
                                    "There is no NodeBGP resource to the endpoint node"
                                ),
                            }
                        }
                    }
                }
                _ => return Err(Error::InvalidExternalTrafficPolicy),
            }
        }
    }

    target_peers.sort();
    target_peers.dedup();

    Ok(target_peers)
}

fn get_svc_name_from_eps(eps: &EndpointSlice) -> Option<&String> {
    eps.labels().get(SERVICE_NAME_LABEL)
}

fn get_svc_peer_groups(svc: &Service) -> Vec<&str> {
    match &svc.annotations().get(PEER_GROUP_ANNOTATION) {
        Some(v) => v.split(',').collect(),
        None => vec![],
    }
}

fn sync_target_peers(peers: &mut BTreeMap<String, AdvertiseStatus>, targets: &[String]) -> bool {
    let mut target_map: HashMap<&str, ()> = HashMap::new();
    let mut updated = false;
    for target in targets.iter() {
        target_map.insert(target, ());
        match peers.get(target) {
            Some(_) => {}
            None => {
                peers.insert(target.clone(), AdvertiseStatus::NotAdvertised);
                updated = true;
            }
        }
    }
    for (k, v) in peers.iter_mut() {
        if target_map.get(k.as_str()).is_none() && (*v).ne(&AdvertiseStatus::Withdraw) {
            *v = AdvertiseStatus::Withdraw;
            updated = true;
        }
    }
    updated
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest(
        peers,
        targets,
        updated,
        expected,
        case(BTreeMap::from([]), vec![], false, BTreeMap::from([])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised)]), vec!["peer1".to_string()], false, BTreeMap::from([])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised), ("peer2".to_string(), AdvertiseStatus::Advertised)]), vec!["peer1".to_string(), "peer2".to_string()], false, BTreeMap::from([])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised), ("peer2".to_string(), AdvertiseStatus::NotAdvertised)]), vec!["peer1".to_string(), "peer2".to_string()], false, BTreeMap::from([])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised)]), vec!["peer1".to_string(), "peer2".to_string()], true, BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised), ("peer2".to_string(), AdvertiseStatus::NotAdvertised)])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised), ("peer2".to_string(), AdvertiseStatus::Advertised)]), vec!["peer2".to_string()], true, BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Withdraw), ("peer2".to_string(), AdvertiseStatus::Advertised)])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised), ("peer2".to_string(), AdvertiseStatus::Advertised)]), vec!["peer2".to_string(), "peer3".to_string()], true, BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Withdraw), ("peer2".to_string(), AdvertiseStatus::Advertised), ("peer3".to_string(), AdvertiseStatus::NotAdvertised)])),
    )]
    fn works_sync_target_peers(
        mut peers: BTreeMap<String, AdvertiseStatus>,
        targets: Vec<String>,
        updated: bool,
        expected: BTreeMap<String, AdvertiseStatus>,
    ) {
        let res = sync_target_peers(&mut peers, &targets);
        assert_eq!(res, updated);
        if res {
            assert_eq!(peers, expected);
        }
    }
}
