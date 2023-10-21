use std::sync::Arc;

use futures::StreamExt;
use kube::{runtime::{controller::Action, finalizer::{finalizer, Event}, watcher::Config, Controller}, Api, ResourceExt, Client, api::ListParams};

use crate::{kubernetes::{crd::{bgp_advertisement::{BGPAdvertisement, BGP_ADVERTISEMENT_FINALIZER}, node_bgp::NodeBGP}, context::{Context, State, error_policy}}, agent::{error::Error, reconciler::node_bgp::ENV_HOSTNAME}};


#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(ba: Arc<BGPAdvertisement>, ctx: Arc<Context>) -> Result<Action, Error> {
	let bgp_advertisements = Api::<BGPAdvertisement>::all(ctx.client.clone());

	finalizer(&bgp_advertisements, BGP_ADVERTISEMENT_FINALIZER, ba, |event| async {
		match event {
			Event::Apply(ba) => reconcile(&bgp_advertisements, &ba, ctx).await,
			Event::Cleanup(ba) => cleanup(&bgp_advertisements, &ba, ctx).await,
		}
	})
	.await
	.map_err(|e| Error::FinalizerError(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(api: &Api<BGPAdvertisement>, ba: &BGPAdvertisement, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(name = ba.name_any(), "Reconcile BGPAdvertisement");

    let node_name = std::env::var(ENV_HOSTNAME).expect("HOSTNAME environment value is not set");

    let node_bgps = Api::<NodeBGP>::all(ctx.client.clone());
    let nb = node_bgps.get(&node_name).await.map_err(Error::KubeError)?;

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(api: &Api<BGPAdvertisement>, ba: &BGPAdvertisement, ctx: Arc<Context>) -> Result<Action, Error> {

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
pub(crate) async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let bgp_advertisements = Api::<BGPAdvertisement>::all(client.clone());
    if let Err(e) = bgp_advertisements.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start BGPAdvertisement reconciler");

    // let node_name = std::env::var(ENV_HOSTNAME).expect("HOSTNAME environment value is not set");
    // let label_selector = format!("{}={}", LABEL_BGP_PEER_NODE, node_name);

    // let watch_config = Config::default().labels(&label_selector);
    let watch_config = Config::default();
    Controller::new(bgp_advertisements, watch_config.any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<BGPAdvertisement, Error>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
