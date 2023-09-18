use std::{sync::Arc, time::Duration};

use futures::StreamExt;
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
use serde_json::json;
use tracing::instrument;

use crate::{context::{Context, State}, error::Error, reconcilers::common::error_policy, bgp::peer};

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "BgpPeer", namespaced)]
#[kube(status = "BgpPeerStatus", shortname = "bgpp")]
pub(crate) struct BgpPeerSpec {
    pub peer: peer::Peer,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct BgpPeerStatus {
    pub status: peer::Status,
}

#[instrument(skip(ctx, resource), fields(trace_id))]
async fn reconcile(resource: Arc<BgpPeer>, ctx: Arc<Context>) -> Result<Action, Error> {

    tracing::info!("BgpPeer reconcile");

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
