use std::{str::FromStr, sync::Arc};

use futures::StreamExt;
use ipnet::IpNet;
use k8s_openapi::api::{
    core::v1::{Node, Service},
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
use time::format_description::modifier::End;

use crate::{
    controller::error::Error,
    kubernetes::{
        context::{error_policy, Context, State},
        crd::{
            address_pool::AddressType,
            bgp_advertisement::{BGPAdvertisement, BGPAdvertisementSpec, Protocol},
            bgp_peer::PEER_GROUP_ANNOTATION,
            node_bgp::NodeBGP,
        },
        owner_reference::create_owner_reference,
    },
};

const ENDPOINTSLICE_FINALIZER: &str = "endpointsliece.sart.terassyi.net/finalizer";
const SERVICE_NAME_LABEL: &str = "kubernetes.io/service-name";

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(eps: Arc<EndpointSlice>, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = eps
        .namespace()
        .ok_or(Error::KubeError(kube::error::Error::Discovery(
            kube::error::DiscoveryError::MissingResource(format!(
                "Namespace for {}",
                eps.name_any()
            )),
        )))?;
    let svc_name = match eps.labels().get(SERVICE_NAME_LABEL) {
        Some(svc) => svc.to_string(),
        None => return Ok(Action::await_change()), // If the service name label is not set, ignore it
    };
    let services = Api::<Service>::namespaced(ctx.client.clone(), &ns);
    let svc = match services
        .get_opt(&svc_name)
        .await
        .map_err(Error::KubeError)?
    {
        Some(svc) => svc,
        None => {
            tracing::warn!(
                name = svc_name,
                "The Service resource associated with Endpointslice is not found"
            );
            return Ok(Action::await_change());
        }
    };
    let service_type = match svc.spec.as_ref() {
        Some(spec) => match spec.type_.as_ref() {
            Some(t) => {
                if t.eq("LoadBalancer") {
                    t.clone()
                } else {
                    "".to_string()
                }
            }
            None => "".to_string(),
        },
        None => "".to_string(),
    };
    if service_type.is_empty() {
        // ignore not LoadBalancers
        return Ok(Action::await_change());
    }

    let endpointslices = Api::<EndpointSlice>::namespaced(ctx.client.clone(), &ns);

    tracing::info!(name=eps.name_any(),obj=?eps,"Reconcile Endpointslice");

    finalizer(
        &endpointslices,
        ENDPOINTSLICE_FINALIZER,
        eps,
        |event| async {
            match event {
                Event::Apply(eps) => reconcile(&eps, &svc, ctx.clone()).await,
                Event::Cleanup(eps) => cleanup(&eps, ctx.clone()).await,
            }
        },
    )
    .await
    .map_err(|e| Error::FinalizerError(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(eps: &EndpointSlice, svc: &Service, ctx: Arc<Context>) -> Result<Action, Error> {
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
    tracing::info!(
        name = eps.name_any(),
        namespace = ns,
        "reconcile Endpointslice"
    );

    let node_bgps = Api::<NodeBGP>::all(ctx.client.clone());
    let target_peers = get_target_peers(eps, svc, &node_bgps).await?;

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
            .map_err(Error::KubeError)?
        {
            Some(adv) => {
                // update if changed
                let mut need_update = false;
                let mut new_adv = adv.clone();

                if adv.spec.cidr.ne(cidr.to_string().as_str()) {
                    tracing::warn!(
                        name = svc.name_any(),
                        namespace = ns,
                        "LoadBalancer address is changed"
                    );
                    new_adv.spec.cidr = cidr.to_string();
                    need_update = true;
                }

                if let Some(peers) = &adv.spec.peers {
                    if peers.len() != target_peers.len() {
                        new_adv.spec.peers = Some(target_peers.clone());
                        need_update = true;
                    } else {
                        for (i, e) in peers.iter().enumerate() {
                            if e.ne(&target_peers[i]) {
                                new_adv.spec.peers = Some(target_peers.clone());
                                need_update = true;
                                break;
                            }
                        }
                    }
                } else if !target_peers.is_empty() {
                    new_adv.spec.peers = Some(target_peers.clone());
                    need_update = true;
                }

                if need_update {
                    tracing::info!(
                        name = adv.name_any(),
                        namespace = ns,
                        "Update BGPAdvertisement"
                    );
                    bgp_advertisements
                        .replace(&new_adv.name_any(), &PostParams::default(), &new_adv)
                        .await
                        .map_err(Error::KubeError)?;
                }
            }
            None => {
                // create new Advertisement
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
                        peers: Some(target_peers.clone()),
                    },
                    status: None,
                };

                tracing::info!(
                    name = adv.name_any(),
                    namespace = ns,
                    "Create BGPAdvertisement"
                );
                bgp_advertisements
                    .create(&PostParams::default(), &adv)
                    .await
                    .map_err(Error::KubeError)?;
            }
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(eps: &EndpointSlice, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(name = eps.name_any(), "cleanup Endpointslice");
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
                        .map_err(Error::KubeError)?;
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
                            match nb_api.get_opt(node_name).await.map_err(Error::KubeError)? {
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

fn get_svc_peer_groups(svc: &Service) -> Vec<&str> {
    match &svc.annotations().get(PEER_GROUP_ANNOTATION) {
        Some(v) => v.split(',').collect(),
        None => vec![],
    }
}
