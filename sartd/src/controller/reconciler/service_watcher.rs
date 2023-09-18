use std::{
    collections::{BTreeMap, HashMap},
    str::FromStr,
    sync::Arc,
};

use futures::StreamExt;
use ipnet::IpNet;
use k8s_openapi::api::{core::v1::Service, discovery::v1::EndpointSlice};
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

use crate::{
    controller::error::Error,
    kubernetes::{
        context::{error_policy, Context, State},
        crd::{
            address_pool::AddressType,
            bgp_advertisement::{
                AdvertiseStatus, BGPAdvertisement, BGPAdvertisementSpec, BGPAdvertisementStatus,
                Protocol,
            },
            bgp_peer::PEER_GROUP_ANNOTATION,
            node_bgp::NodeBGP,
        },
        util::{create_owner_reference, get_namespace},
    },
};

const ENDPOINTSLICE_FINALIZER: &str = "endpointsliece.sart.terassyi.net/finalizer";
const SERVICE_FINALIZER: &str = "service.sart.terassyi.net/finalizer";
const SERVICE_NAME_LABEL: &str = "kubernetes.io/service-name";

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(eps: Arc<EndpointSlice>, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<EndpointSlice>(&eps).map_err(Error::KubeLibrary)?;

    let endpointslices = Api::<EndpointSlice>::namespaced(ctx.client.clone(), &ns);

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
    let services = Api::<Service>::namespaced(ctx.client.clone(), &ns);
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

    let node_bgps = Api::<NodeBGP>::all(ctx.client.clone());
    let target_peers = get_target_peers(eps, &svc, &node_bgps).await?;

    let adv_cidrs = match svc.status.clone().and_then(|lb| {
        lb.load_balancer.and_then(|lb_status| {
            lb_status.ingress.map(|ingresses| {
                ingresses
                    .iter()
                    .filter_map(|ingress| ingress.ip.clone())
                    .collect::<Vec<String>>()
            })
        })
    }) {
        Some(cidrs) => cidrs,
        None => {
            tracing::warn!(
                name = svc.name_any(),
                namespace = ns,
                "There is no lb addresses"
            );
            // for development, insert dummy address
            vec!["0.0.0.0/32".to_string()]
            // return
        }
    };

    let bgp_advertisements = Api::<BGPAdvertisement>::namespaced(ctx.client.clone(), ns.as_str());

    for adv_cidr in adv_cidrs.iter() {
        let cidr = IpNet::from_str(adv_cidr).map_err(|_| Error::InvalidAddress)?;
        let adv_name = format!("{}-{}", eps.name_any(), Protocol::from(&cidr));

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
                    return Ok(Action::await_change());
                }

                if let Some(current_targets) = new_adv
                    .status
                    .as_mut()
                    .and_then(|status| status.peers.as_mut())
                {
                    let need_update = sync_target_peers(current_targets, &target_peers);
                    tracing::info!(name = adv.name_any(), namespace = ns, tagets =? target_peers, status =? new_adv.status, "Sync peers");
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
                tracing::info!(name = adv.name_any(), namespace = ns, tagets =? target_peers, status =? new_adv.status, "Sync peers");
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
            }
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(eps: &EndpointSlice, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<EndpointSlice>(eps).map_err(Error::KubeLibrary)?;

    tracing::info!(
        name = eps.name_any(),
        namespace = ns,
        "Cleanup Endpointslice"
    );

    Ok(Action::await_change())
}

pub(crate) async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let endpointslices = Api::<EndpointSlice>::all(client.clone());

    tracing::info!("Start Endpointslice watcher");

    Controller::new(endpointslices, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<EndpointSlice, Error>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

async fn add_service_finalizer(svc: &Service) -> Result<(), Error> {
    Ok(())
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

fn is_loadbalacner(svc: &Service) -> bool {
    match svc.spec.as_ref().and_then(|spec| spec.type_.as_ref()) {
        Some(t) => t.eq("LoadBalancer"),
        None => false,
    }
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
