use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use kube::{CustomResource, runtime::{controller::Action, watcher::Config, Controller}, Client, Api, api::ListParams};
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};
use tracing::instrument;

use crate::{context::{Context, State}, error::Error, reconcilers::common::error_policy};


#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "AddressPool")]
#[kube(status = "AddressPoolStatus")]
pub(crate) struct AddressPoolSpec {
	pub r#type: String,
	pub cidr: String,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct AddressPoolStatus {}

#[instrument(skip(ctx, resource), fields(trace_id))]
async fn reconcile(resource: Arc<AddressPool>, ctx: Arc<Context>) -> Result<Action, Error> {

    tracing::info!("Reconcile AddressPool");

    Ok(Action::requeue(Duration::from_secs(5 * 60)))
}

#[instrument()]
pub(crate) async fn run(state: State, interval: u64) {
    let client = Client::try_default().await.expect("failed to create kube Client");

    let address_pool = Api::<AddressPool>::all(client.clone());
    if let Err(e) = address_pool.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Starting AddressPool reconciler");

    Controller::new(address_pool, Config::default().any_semantic())
    .shutdown_on_signal()
    .run(reconcile, error_policy::<AddressPool>, state.to_context(client, interval))
    .filter_map(|x| async move { std::result::Result::ok(x) })
    .for_each(|_| futures::future::ready(()))
    .await;
}
