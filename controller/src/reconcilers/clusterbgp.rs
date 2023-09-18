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
    CustomResource, Resource, core::DynamicResourceScope,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;
use tracing::instrument;



use crate::{context::{Context, State}, error::Error, bgp::peer};

use super::{common::error_policy, bgppeer::BgpPeer};


#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "ClusterBgp")]
#[kube(status = "ClusterBgpStatus")]
pub(crate) struct ClusterBgpSpec {
    pub policy: String,
    pub peers: Vec<peer::Peer>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct ClusterBgpStatus {

}

#[instrument(skip(ctx, resource), fields(trace_id))]
async fn reconcile(resource: Arc<ClusterBgp>, ctx: Arc<Context>) -> Result<Action, Error> {

    tracing::info!("ClusterBgp reconcile");

    Ok(Action::requeue(Duration::from_secs(5)))
}

#[instrument()]
pub(crate) async fn run(state: State) {
    let client = Client::try_default().await.expect("failed to create kube Client");

    let cluster_bgps = Api::<ClusterBgp>::all(client.clone());
    if let Err(e) = cluster_bgps.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Starting ClusterBgp reconciler");

    Controller::new(cluster_bgps, Config::default().any_semantic())
    .shutdown_on_signal()
    .run(reconcile, error_policy::<ClusterBgp>, state.to_context(client))
    .filter_map(|x| async move { std::result::Result::ok(x) })
    .for_each(|_| futures::future::ready(()))
    .await;
}
