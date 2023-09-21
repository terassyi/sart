use std::{sync::Arc, time::Duration, net::IpAddr, str::FromStr};

use chrono::Utc;
use futures::{StreamExt};
use futures::stream::{self, TryStreamExt};
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{Api, ListParams, ResourceExt},
    client::Client,
    runtime::{
        controller::{Action, Controller},
        watcher::{Config, self}, WatchStreamExt,
    },
    CustomResource,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tracing::{instrument, field, Span};

use crate::reconcilers::common;
use crate::{context::{Context, State}, error::Error, reconcilers::common::{error_policy, DEFAULT_RECONCILE_REQUEUE_INTERVAL}, bgp::peer::{self, BGP_PORT, SpeakerType}, speaker::{sart::{CLIENT_TIMEOUT, SartSpeaker}, speaker::{self, DEFAULT_ENDPOINT_CONNECT_TIMEOUT, Speaker}}};

use crate::telemetry;

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "BgpPeer")]
#[kube(status = "BgpPeerStatus", shortname = "bgpp")]
#[serde(rename_all = "camelCase")]
pub(crate) struct BgpPeerSpec {
    pub peer: peer::Peer,
    pub r#type: SpeakerType,
    pub endpoint_timeout: u64,
    pub advertisements: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct BgpPeerStatus {
    pub status: peer::Status,
}

#[instrument(skip(ctx, resource), fields(trace_id))]
async fn reconcile(resource: Arc<BgpPeer>, ctx: Arc<Context>) -> Result<Action, Error> {

    let trace_id = telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));
    let _timer = ctx.metrics.count_and_measure();
    ctx.diagnostics.write().await.last_event = Utc::now();

    tracing::info!("Reconcile BgpPeer");

    let peers: Api<BgpPeer> = Api::all(ctx.client.clone());
    if let Err(e) = peers.get(&resource.name_any()).await {
        // If any BgpPeer does'nt exist,
        // return error
        tracing::error!(error=?e,name=&resource.name_any(),"BgpPeer resource is not found");
        return Err(Error::KubeError(e));
    }

    // Check finalizer
    if let Some(_) = resource.finalizers().iter().find(|&s|s.eq(&common::finalizer("bgppeer"))) {
        // If `bgppeer.sart.terassyi.net/finalizer` is set, shutdown the BGP session and delete its resource.

        tracing::info!(name=resource.name_any(),"finalizer is set. delete it");


        return Ok(Action::requeue(Duration::from_secs(ctx.interval)));
    }

    // handle new or updated resource

    // check wether the same bgp session(some resource that has same local_asn and local_addr, neighbor).

    // If such one exists, abort creation request.
    // This validation should be done by a validation webhook...
    // TODO: do it by a validation webhook

    // Create new bgp session.
    let speaker_addr: IpAddr = match &resource.spec.peer.local_addr {
        Some(addr) => {
            addr.parse().unwrap()
        },
        None => {
            let nodes: Api<Node> = Api::all(ctx.client.clone());
            let node_name = resource.spec.peer.local_name.as_ref().ok_or(Error::InvalidParameter(String::from("bgppeer.spec.local_name")))?;
            let speaker_node = nodes.get(node_name).await.map_err(Error::KubeError)?;

            get_node_internal_addr(speaker_node).ok_or(Error::AddressNotFound)?
        }
    };

    // Call the bgp speaker's peer creation endpoint.
    let endpoint = format!("{}:{}", speaker_addr.to_string(), BGP_PORT);

    // create API client for the speaker
    tracing::info!(endpoint=endpoint,"create speaker endpoint");
    let speaker_client = match resource.spec.r#type {
        SpeakerType::Sart => {
            let timeout = if resource.spec.endpoint_timeout == 0 {
                DEFAULT_ENDPOINT_CONNECT_TIMEOUT
            } else {
                resource.spec.endpoint_timeout
            };
            speaker::new::<SartSpeaker>(&endpoint, timeout)
        },
    };
    tracing::info!(peer_asn=resource.spec.peer.neighbor.asn,peer_addr=resource.spec.peer.neighbor.addr,"create BGP peer");
    speaker_client.add_peer(resource.spec.peer.clone()).await?;

    let peer_addr = IpAddr::from_str(&resource.spec.peer.neighbor.addr).map_err(|_| Error::InvalidParameter("bgppeer.spec.peer.neighbor.addr".to_string()))?;
    let res = speaker_client.get_peer(peer_addr).await?;

    tracing::info!(peer_asn=res.neighbor.asn,peer_addr=res.neighbor.addr,state=?res.state,"get peer");

    // Check that bgp session is established.
    if res.state.unwrap().ne(&peer::Status::Established) {
        // by calling endpoint or requeue.
        return Ok(Action::requeue(Duration::from_secs(1)));
    }


    Ok(Action::requeue(Duration::from_secs(DEFAULT_RECONCILE_REQUEUE_INTERVAL)))
}

#[instrument()]
pub(crate) async fn run(state: State, interval: u64) {
    let client = Client::try_default().await.expect("failed to create kube Client");

    let bgp_peers = Api::<BgpPeer>::all(client.clone());
    if let Err(e) = bgp_peers.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Starting ClusterBgp reconciler");

    Controller::new(bgp_peers, Config::default().any_semantic())
    .shutdown_on_signal()
    .run(reconcile, error_policy::<BgpPeer>, state.to_context(client, interval))
    .filter_map(|x| async move { std::result::Result::ok(x) })
    .for_each(|_| futures::future::ready(()))
    .await;
}

fn get_node_internal_addr(node: Node) -> Option<IpAddr> {
    if let Some(status) = node.status {
        if let Some(addrs) = status.addresses {
            addrs.iter().find(|addr| addr.type_.eq("InternalIP")).map(|addr| IpAddr::from_str(&addr.address).ok()).flatten()
        } else {
            None
        }
    } else {
        None
    }
}
