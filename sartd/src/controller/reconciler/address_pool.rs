use std::sync::Arc;

use futures::StreamExt;
use kube::{
    api::ListParams,
    runtime::{
        controller::{Action, Controller},
        finalizer::{finalizer, Event},
        watcher::Config,
    },
    Api, Client, ResourceExt,
};

use crate::{
    controller::error::Error,
    kubernetes::{
        context::{error_policy, Context, State},
        crd::address_pool::{AddressPool, ADDRESS_POOL_FINALIZER},
    },
};

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(ap: Arc<AddressPool>, ctx: Arc<Context>) -> Result<Action, Error> {
    let address_pools = Api::<AddressPool>::all(ctx.client.clone());

    finalizer(&address_pools, ADDRESS_POOL_FINALIZER, ap, |event| async {
        match event {
            Event::Apply(ap) => reconcile(&ap, ctx.clone()).await,
            Event::Cleanup(ap) => cleanup(&ap, ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::FinalizerError(Box::new(e)))
}

#[tracing::instrument(skip_all)]
async fn reconcile(ap: &AddressPool, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(name = ap.name_any(), "reconcile AddressPool");

	Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup(ap: &AddressPool, ctx: Arc<Context>) -> Result<Action, Error> {

	Ok(Action::await_change())
}

pub(crate) async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let address_pools = Api::<AddressPool>::all(client.clone());
    if let Err(e) = address_pools.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start ClusterBgp reconciler");

    Controller::new(address_pools, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<AddressPool, Error>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
