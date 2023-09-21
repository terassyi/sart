use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use kube::{CustomResource, runtime::{controller::Action, watcher::Config, Controller}, Client, Api, api::ListParams};
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};
use tracing::instrument;

use crate::{context::{Context, State}, error::Error, reconcilers::common::error_policy, bgp::{advertisement, common}};


#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "BgpAdvertisement")]
#[kube(status = "BgpAdvertisementStatus")]
#[serde(rename_all = "camelCase")]
pub(crate) struct BgpAdvertisementSpec {
	pub network: String,
    pub r#type: advertisement::Type,
    pub protocol: common::Protocol,
    pub origin: Option<advertisement::Origin>,
    pub local_pref: Option<advertisement::LocalPref>,
    pub peers: Vec<String>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct BgpAdvertisementStatus {
    pub status: advertisement::Status,
}

#[instrument(skip(ctx, resource), fields(trace_id))]
async fn reconcile(resource: Arc<BgpAdvertisement>, ctx: Arc<Context>) -> Result<Action, Error> {

    tracing::info!("Reconcile BgpAdvertisement");

    Ok(Action::requeue(Duration::from_secs(5 * 60)))
}

#[instrument()]
pub(crate) async fn run(state: State, interval: u64) {
    let client = Client::try_default().await.expect("failed to create kube Client");

    let bgp_advertisements = Api::<BgpAdvertisement>::all(client.clone());
    if let Err(e) = bgp_advertisements.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Starting BgpAdvertisement reconciler");

    Controller::new(bgp_advertisements, Config::default().any_semantic())
    .shutdown_on_signal()
    .run(reconcile, error_policy::<BgpAdvertisement>, state.to_context(client, interval))
    .filter_map(|x| async move { std::result::Result::ok(x) })
    .for_each(|_| futures::future::ready(()))
    .await;
}
