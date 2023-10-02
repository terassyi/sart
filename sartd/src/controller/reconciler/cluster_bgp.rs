use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use kube::{runtime::{controller::{Action, Controller}, watcher::Config}, Client, Api, api::ListParams, ResourceExt};

use crate::{kubernetes::{crd::cluster_bgp::ClusterBGP, context::{Context, State, error_policy}}, controller::error::Error};


#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(cb: Arc<ClusterBGP>, ctx: Arc<Context>) -> Result<Action, Error> {

	tracing::info!(name=cb.name_any(),"Reconcile ClusterBGP");

    Ok(Action::requeue(Duration::from_secs(5)))
}

pub(crate) async fn run(state: State, interval: u64) {
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
		.run(reconcile, error_policy::<ClusterBGP, Error>, state.to_context(client, interval))
		.filter_map(|x| async move {
			std::result::Result::ok(x)
		})
		.for_each(|_| futures::future::ready(()))
		.await;
}
