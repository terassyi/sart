use std::{sync::Arc, time::Duration, net::{Ipv4Addr, IpAddr}, str::FromStr};

use chrono::Utc;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{Api, ListParams, Patch, PatchParams, ResourceExt},
    client::Client,
    runtime::{
        controller::{Action, Controller},
        events::{Event, EventType, Recorder, Reporter},
        finalizer::{finalizer, Event as Finalizer},
        watcher::Config,
    },
    CustomResource, Resource,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, from_str};
use tracing::{instrument, field, Span};

use crate::{context::{Context, State}, error::Error, reconcilers::common::error_policy, bgp::peer::{self, BGP_PORT}, rpc::client::{self, CLIENT_TIMEOUT}};
use crate::proto;
use crate::telemetry;

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "BgpPeer")]
#[kube(status = "BgpPeerStatus", shortname = "bgpp")]
pub(crate) struct BgpPeerSpec {
    pub peer: peer::Peer,
    pub advertisements: Vec<String>,
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

    tracing::info!("BgpPeer reconcile");

    let peers: Api<BgpPeer> = Api::all(ctx.client.clone());
    if let Ok(peer) = peers.get(&resource.name_any()).await {
        // If BgpPeer resource already exists.

        // Check diffs

        // If there are some diffs, drop bgp session and reconnect new one.
        // And updates BgpPeerStatus to NotEstablished.

        // If there is no diff, check wether bgp session status is established.
        // call bgp speaker's endpoint to check status and sync it.
        
        // If the bgp session is not established, requeue after `backoff_interval` seconds.
        // And update BgpPeerStatus reason based on the result endpoint call.
    }

    // If any BgpPeer does'nt exist,
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
    let client = tokio::time::timeout(Duration::from_secs(CLIENT_TIMEOUT), client::connect_bgp(&endpoint))
        .await
        .map_err(|_| Error::ClientTimeout)?;

    let res = client.add_peer(proto::sart::AddPeerRequest{
        peer: proto::sart::Peer{
            asn: resource.spec.peer.neighbor.asn,
            address: resource.spec.peer.neighbor.addr.
        }
    });

    // Check that bgp session is established.
    // by calling endpoint or requeue.


    Ok(Action::requeue(Duration::from_secs(5)))
}

#[instrument()]
pub(crate) async fn run(state: State) {
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
    .run(reconcile, error_policy::<BgpPeer>, state.to_context(client))
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
