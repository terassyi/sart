use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::ListParams,
    runtime::{controller::Action, watcher::Config, Controller},
    Api, Client, ResourceExt,
};

use crate::{
    context::{Context, State},
    error::Error,
    reconcilers::{
        clusterbgp::ClusterBgp,
        common::{error_policy, get_match_labels},
    },
};

use super::common::DEFAULT_RECONCILE_REQUEUE_INTERVAL;

#[tracing::instrument(skip(ctx, resource), fields(trace_id))]
async fn reconcile(resource: Arc<Node>, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!("Reconcile Node");

    // Check if ClusterBgp resoruce exists.
    let cluster_bgps = Api::<ClusterBgp>::all(ctx.client.clone());

    let cluster_bgp_list = cluster_bgps
        .list(&ListParams::default())
        .await
        .map_err(Error::KubeError)?;
    if cluster_bgp_list.items.is_empty() {
        tracing::info!("ClusterBgp resource doesn't exist");
        return Ok(Action::requeue(Duration::from_secs(
            DEFAULT_RECONCILE_REQUEUE_INTERVAL,
        )));
    }

    for cb in cluster_bgp_list.iter() {
        // Matching ClusterBgp's node selector
        // FIXME: supports matchExpressions
        tracing::info!("list ClusterBgp resource");
        let matched_labels = get_match_labels(cb);
        tracing::info!(matched_labels=?matched_labels,name=cb.name_any());
    }

    Ok(Action::requeue(Duration::from_secs(
        DEFAULT_RECONCILE_REQUEUE_INTERVAL,
    )))
}

pub(crate) async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");

    let nodes = Api::<Node>::all(client.clone());
    if let Err(e) = nodes.list(&ListParams::default().limit(1)).await {
        tracing::error!("Node resoruce is not found.");
        std::process::exit(1);
    }

    tracing::info!("Starting AddressPool reconciler");

    Controller::new(nodes, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconcile,
            error_policy::<Node>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
