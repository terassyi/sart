use std::{collections::BTreeMap, net::Ipv4Addr, sync::Arc, time::Duration};

use futures::StreamExt;
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{ListParams, PostParams},
    core::ObjectMeta,
    runtime::{
        controller::{Action, Controller},
        finalizer::{finalizer, Event},
        watcher::Config,
    },
    Api, Client, ResourceExt,
};
use tracing::{field, Span};

use crate::{
    context::{error_policy, Context, State},
    controller::error::Error,
    crd::{
        bgp_peer::{BGPPeerSlim, PeerConfig},
        bgp_peer_template::BGPPeerTemplate,
        cluster_bgp::{
            AsnSelectionType, AsnSelector, ClusterBGP, ClusterBGPStatus, RouterIdSelectionType,
            RouterIdSelector, ASN_LABEL, CLUSTER_BGP_FINALIZER, ROUTER_ID_LABEL,
        },
        node_bgp::{NodeBGP, NodeBGPSpec, NodeBGPStatus},
    },
    util::{create_owner_reference, diff},
};

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(cb: Arc<ClusterBGP>, ctx: Arc<Context>) -> Result<Action, Error> {
    let cluster_bgps = Api::<ClusterBGP>::all(ctx.client.clone());

    finalizer(&cluster_bgps, CLUSTER_BGP_FINALIZER, cb, |event| async {
        match event {
            Event::Apply(cb) => reconcile(&cb, ctx.clone()).await,
            Event::Cleanup(cb) => cleanup(&cb, ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

// reconcile() is called when a resource is applied or updated
#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(cb: &ClusterBGP, ctx: Arc<Context>) -> Result<Action, Error> {
    let trace_id = sartd_trace::telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));

    tracing::info!(name = cb.name_any(), "reconcile ClusterBGP");

    let mut need_requeue = false;

    let nodes = Api::<Node>::all(ctx.client.clone());
    let list_params = match &cb.spec.node_selector {
        Some(selector) => ListParams::default().labels(&get_label_selector(selector)),
        None => ListParams::default(),
    };

    let actual_nodes = match cb
        .status
        .clone()
        .map(|status| status.nodes.unwrap_or_default())
    {
        Some(nodes) => nodes,
        None => Vec::new(),
    };

    let matched_nodes = nodes.list(&list_params).await.map_err(Error::Kube)?;
    let matched_node_names = matched_nodes
        .iter()
        .map(|n| n.name_any())
        .collect::<Vec<String>>();

    let (added, remain, removed) = diff(&actual_nodes, &matched_node_names);

    let node_bgps = Api::<NodeBGP>::all(ctx.client.clone());

    for node_name in added.iter() {
        let node = matched_nodes
            .iter()
            .find(|n| n.name_any().eq(node_name.as_str()))
            .ok_or(Error::NodeNotFound)?;

        let asn = get_asn(node, &cb.spec.asn_selector)?;
        let router_id = get_router_id(node, &cb.spec.router_id_selector)?;

        match node_bgps
            .get_opt(&node.name_any())
            .await
            .map_err(Error::Kube)?
        {
            Some(nb) => {
                let mut new_nb = nb.clone();
                let mut need_status_update = true;
                let mut need_spec_update = false;
                match new_nb.status.as_mut() {
                    Some(status) => match status.cluster_bgp_refs.as_mut() {
                        Some(cb_refs) => {
                            if !cb_refs.iter().any(|c| c.eq(cb.name_any().as_str())) {
                                cb_refs.push(cb.name_any());
                                cb_refs.sort();
                            } else {
                                need_status_update = false;
                            }
                        }
                        None => {
                            status.cluster_bgp_refs = Some(vec![cb.name_any()]);
                        }
                    },
                    None => {
                        new_nb.status = Some(NodeBGPStatus {
                            backoff: 0,
                            cluster_bgp_refs: Some(vec![cb.name_any()]),
                            conditions: None,
                        })
                    }
                }

                let peer_templ_api = Api::<BGPPeerTemplate>::all(ctx.client.clone());
                let peers = get_peers(cb, &new_nb, &peer_templ_api).await?;
                match new_nb.spec.peers.as_mut() {
                    Some(nb_peers) => {
                        for peer in peers.iter() {
                            if !nb_peers.iter().any(|p| p.name.eq(&peer.name)) {
                                nb_peers.push(peer.clone());
                                need_spec_update = true;
                            }
                        }
                    }
                    None => {
                        new_nb.spec.peers = Some(peers);
                        need_spec_update = true;
                    }
                }
                if need_spec_update {
                    tracing::info!(node_bgp=nb.name_any(),asn=nb.spec.asn,router_id=?nb.spec.router_id,"update existing NodeBGP's spec.peers");
                    node_bgps
                        .replace(&nb.name_any(), &PostParams::default(), &new_nb)
                        .await
                        .map_err(Error::Kube)?;
                }
                if need_status_update {
                    tracing::info!(node_bgp=nb.name_any(),asn=nb.spec.asn,router_id=?nb.spec.router_id,"update existing NodeBGP's status");
                    node_bgps
                        .replace_status(
                            &nb.name_any(),
                            &PostParams::default(),
                            serde_json::to_vec(&new_nb).map_err(Error::Serialization)?,
                        )
                        .await
                        .map_err(Error::Kube)?;
                }
            }
            None => {
                let mut nb = NodeBGP {
                    metadata: ObjectMeta {
                        name: Some(node.name_any()),
                        labels: Some(node.labels().clone()),
                        owner_references: Some(vec![create_owner_reference(node)]),
                        ..Default::default()
                    },
                    spec: NodeBGPSpec {
                        asn,
                        router_id: router_id.to_string(),
                        speaker: cb.spec.speaker.clone(),
                        peers: None,
                    },
                    status: Some(NodeBGPStatus {
                        backoff: 0,
                        ..Default::default()
                    }),
                };

                let peer_templ_api = Api::<BGPPeerTemplate>::all(ctx.client.clone());
                let peers = get_peers(cb, &nb, &peer_templ_api).await?;

                nb.spec.peers = Some(peers);

                tracing::info!(node_bgp=node.name_any(),asn=asn,router_id=?router_id,"create new NodeBGP resource");
                node_bgps
                    .create(&PostParams::default(), &nb)
                    .await
                    .map_err(Error::Kube)?;
                need_requeue = true;
            }
        }
    }

    for node_name in removed.iter() {
        // matched_nodes doesn't contain target node.
        if let Some(nb) = node_bgps.get_opt(node_name).await.map_err(Error::Kube)? {
            let mut new_nb = nb.clone();
            let mut need_status_update = false;
            let mut need_spec_update = false;

            if let Some(status) = new_nb.status.as_mut() {
                if let Some(cb_refs) = status.cluster_bgp_refs.as_mut() {
                    if !cb_refs.iter().any(|c| c.eq(cb.name_any().as_str())) {
                        cb_refs.retain(|c| c.ne(cb.name_any().as_str()));
                        need_status_update = true;
                    }
                }
            }

            let peer_templ_api = Api::<BGPPeerTemplate>::all(ctx.client.clone());
            let peers = get_peers(cb, &nb, &peer_templ_api).await?;

            if let Some(nb_peers) = new_nb.spec.peers.as_mut() {
                for peer in peers.iter() {
                    if nb_peers.iter().any(|p| p.name.eq(&peer.name)) {
                        nb_peers.retain(|p| p.name.ne(&peer.name));
                        need_spec_update = true;
                    }
                }
            }

            if need_spec_update {
                tracing::info!(node_bgp=nb.name_any(),asn=nb.spec.asn,router_id=?nb.spec.router_id,"update existing NodeBGP's spec.peers");
                node_bgps
                    .replace(&nb.name_any(), &PostParams::default(), &new_nb)
                    .await
                    .map_err(Error::Kube)?;
            }

            if need_status_update {
                tracing::info!(node_bgp=nb.name_any(),asn=nb.spec.asn,router_id=?nb.spec.router_id,"Update existing NodeBGP's status");
                node_bgps
                    .replace_status(
                        &nb.name_any(),
                        &PostParams::default(),
                        serde_json::to_vec(&new_nb).map_err(Error::Serialization)?,
                    )
                    .await
                    .map_err(Error::Kube)?;
            }
        }
    }

    let mut new_belonging_nodes: Vec<String> = added.iter().map(|n| n.to_string()).collect();
    let mut remain_nodes: Vec<String> = remain.iter().map(|n| n.to_string()).collect();
    new_belonging_nodes.append(&mut remain_nodes);

    if !added.is_empty() || !removed.is_empty() {
        let cluster_bgp_api = Api::<ClusterBGP>::all(ctx.client.clone());
        let mut new_cb = cb.clone();
        new_belonging_nodes.sort();
        match new_cb.status.as_mut() {
            Some(status) => {
                status.nodes = Some(new_belonging_nodes);
            }
            None => {
                new_cb.status = Some(ClusterBGPStatus {
                    desired_nodes: Some(new_belonging_nodes.clone()),
                    nodes: Some(new_belonging_nodes),
                })
            }
        };

        cluster_bgp_api
            .replace_status(
                &cb.name_any(),
                &PostParams::default(),
                serde_json::to_vec(&new_cb).map_err(Error::Serialization)?,
            )
            .await
            .map_err(Error::Kube)?;
    }

    if need_requeue {
        return Ok(Action::requeue(Duration::from_secs(1)));
    }

    Ok(Action::await_change())
}

// cleanup() is called when a resource is deleted
#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(cb: &ClusterBGP, _ctx: Arc<Context>) -> Result<Action, Error> {
    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let cluster_bgps = Api::<ClusterBGP>::all(client.clone());
    if let Err(e) = cluster_bgps.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start ClusterBgp reconciler");

    Controller::new(cluster_bgps, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<ClusterBGP, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

fn get_label_selector(selector: &BTreeMap<String, String>) -> String {
    selector
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<String>>()
        .join(",")
}

fn get_asn(node: &Node, selector: &AsnSelector) -> Result<u32, Error> {
    match selector.from {
        AsnSelectionType::Label => node
            .labels()
            .get(ASN_LABEL)
            .ok_or(Error::AsnNotFound)?
            .parse()
            .map_err(|_| Error::InvalidAsnValue),
        AsnSelectionType::Asn => selector.asn.ok_or(Error::InvalidAsnValue),
    }
}

fn get_router_id(node: &Node, selector: &RouterIdSelector) -> Result<Ipv4Addr, Error> {
    match selector.from {
        RouterIdSelectionType::RouterId => selector
            .router_id
            .clone()
            .ok_or(Error::AddressNotFound)?
            .parse::<Ipv4Addr>()
            .map_err(|_| Error::InvalidAddress),
        RouterIdSelectionType::InternalAddress => node
            .status
            .clone()
            .ok_or(Error::FailedToGetData("node.status".to_string()))?
            .addresses
            .ok_or(Error::FailedToGetData("node.status.addresses".to_string()))?
            .iter()
            .find(|&na| na.type_.eq("InternalIP"))
            .ok_or(Error::AddressNotFound)?
            .address
            .parse()
            .map_err(|_| Error::InvalidAddress),
        RouterIdSelectionType::Label => node
            .labels()
            .get(ROUTER_ID_LABEL)
            .ok_or(Error::AddressNotFound)?
            .parse()
            .map_err(|_| Error::InvalidAddress),
    }
}

fn match_selector(labels: &BTreeMap<String, String>, selector: &BTreeMap<String, String>) -> bool {
    for (k, v) in selector.iter() {
        match labels.get_key_value(k) {
            Some((_mk, mv)) => {
                if mv.ne(v) {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

async fn get_peers(
    cb: &ClusterBGP,
    nb: &NodeBGP,
    peer_templ_api: &Api<BGPPeerTemplate>,
) -> Result<Vec<BGPPeerSlim>, Error> {
    let peer_configs: Vec<PeerConfig> = match &cb.spec.peers {
        Some(peer_config) => peer_config
            .iter()
            .filter(|&p| match &p.node_bgp_selector {
                Some(selector) => match_selector(nb.labels(), selector),
                None => true,
            })
            .cloned()
            .collect(),
        None => Vec::new(),
    };
    let mut peers = Vec::new();
    for c in peer_configs.iter() {
        let mut p = c
            .to_peer(&nb.name_any(), peer_templ_api)
            .await
            .map_err(Error::CRD)?;
        p.spec.cluster_bgp_ref = Some(cb.name_any());
        peers.push(p);
    }
    Ok(peers)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use std::collections::BTreeMap;

    use super::get_label_selector;
    use super::match_selector;

    #[rstest(
		selector,
		expected,
		case(BTreeMap::from([]), ""),
		case(BTreeMap::from([("bgp".to_string(), "a".to_string())]), "bgp=a"),
		case(BTreeMap::from([("bgp".to_string(), "a".to_string()), ("test".to_string(), "b".to_string())]), "bgp=a,test=b"),
	)]
    fn test_get_label_selector(selector: BTreeMap<String, String>, expected: &str) {
        let res = get_label_selector(&selector);
        assert_eq!(res, expected);
    }

    #[rstest(
        labels,
        selector,
        expected,
        case(BTreeMap::from([]), BTreeMap::from([]), true),
        case(BTreeMap::from([("bgp".to_string(), "a".to_string())]), BTreeMap::from([]), true),
        case(BTreeMap::from([]), BTreeMap::from([("bgp".to_string(), "a".to_string())]), false),
        case(BTreeMap::from([("bgp".to_string(), "b".to_string())]), BTreeMap::from([("bgp".to_string(), "a".to_string())]), false),
        case(BTreeMap::from([("bgp".to_string(), "b".to_string()), ("peer".to_string(), "c".to_string())]), BTreeMap::from([("bgp".to_string(), "b".to_string())]), true),
        case(BTreeMap::from([("bgp".to_string(), "b".to_string()), ("peer".to_string(), "c".to_string())]), BTreeMap::from([("bgp".to_string(), "b".to_string()), ("peer".to_string(), "d".to_string())]), false),
    )]
    fn test_match_selector(
        labels: BTreeMap<String, String>,
        selector: BTreeMap<String, String>,
        expected: bool,
    ) {
        let res = match_selector(&labels, &selector);
        assert_eq!(res, expected);
    }
}
