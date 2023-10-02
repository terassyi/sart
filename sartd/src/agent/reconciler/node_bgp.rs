use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use kube::{runtime::{controller::{Action, Controller}, watcher::Config}, Client, Api, api::ListParams};

use crate::{kubernetes::{crd::node_bgp::NodeBGP, context::{Context, State, error_policy}}, controller::error::Error};


#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(nb: Arc<NodeBGP>, ctx: Arc<Context>) -> Result<Action, Error> {

    Ok(Action::requeue(Duration::from_secs(5)))
}

pub(crate) async fn run(state: State, interval: u64) {
	let client = Client::try_default()
		.await
		.expect("Failed to create kube client");

	let node_bgps = Api::<NodeBGP>::all(client.clone());
	if let Err(e) = node_bgps.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
	}

    tracing::info!("Start NodeBGP reconciler");

	Controller::new(node_bgps, Config::default().any_semantic())
		.shutdown_on_signal()
		.run(reconcile, error_policy::<NodeBGP, Error>, state.to_context(client, interval))
		.filter_map(|x| async move {
			std::result::Result::ok(x)
		})
		.for_each(|_| futures::future::ready(()))
		.await;
}
