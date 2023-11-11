use std::sync::Arc;

use futures::StreamExt;
use kube::{
    api::{ListParams, PostParams},
    runtime::{
        controller::Action,
        finalizer::{finalizer, Event},
        watcher::Config,
        Controller,
    },
    Api, Client, ResourceExt,
};

use crate::{
    controller::error::Error,
    kubernetes::{
        context::{error_policy, Context, State},
        crd::bgp_advertisement::{AdvertiseStatus, BGPAdvertisement, BGP_ADVERTISEMENT_FINALIZER},
        util::get_namespace,
    },
};

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(ba: Arc<BGPAdvertisement>, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<BGPAdvertisement>(&ba).map_err(Error::KubeLibrary)?;
    let bgp_advertisements = Api::<BGPAdvertisement>::namespaced(ctx.client.clone(), &ns);

    finalizer(
        &bgp_advertisements,
        BGP_ADVERTISEMENT_FINALIZER,
        ba,
        |event| async {
            match event {
                Event::Apply(ba) => reconcile(&bgp_advertisements, &ba, ctx).await,
                Event::Cleanup(ba) => cleanup(&bgp_advertisements, &ba, ctx).await,
            }
        },
    )
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(
    api: &Api<BGPAdvertisement>,
    ba: &BGPAdvertisement,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    let ns = get_namespace::<BGPAdvertisement>(ba).map_err(Error::KubeLibrary)?;
    tracing::info!(
        name = ba.name_any(),
        namespace = ns,
        "Reconcile BGPAdvertisement"
    );

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(
    api: &Api<BGPAdvertisement>,
    ba: &BGPAdvertisement,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    let ns = get_namespace::<BGPAdvertisement>(ba).map_err(Error::KubeLibrary)?;
    tracing::info!(
        name = ba.name_any(),
        namespace = ns,
        "Cleanup BGPAdvertisement"
    );

    let mut new_ba = ba.clone();
    let mut need_update = false;
    if let Some(peers) = new_ba
        .status
        .as_mut()
        .and_then(|status| status.peers.as_mut())
    {
        if peers.is_empty() {
            tracing::info!(
                name = ba.name_any(),
                namespace = ns,
                "Successfully delete BGPAdvertisement"
            );
            return Ok(Action::await_change());
        }
        for (_p, s) in peers.iter_mut() {
            *s = AdvertiseStatus::Withdraw;
            need_update = true;
        }
    }

    if need_update {
        api.replace_status(
            &ba.name_any(),
            &PostParams::default(),
            serde_json::to_vec(&new_ba).map_err(Error::Serialization)?,
        )
        .await
        .map_err(Error::Kube)?;

        tracing::info!(
            name = &ba.name_any(),
            namespace = ns,
            "Submit withdraw request"
        );
    }

    Err(Error::Withdrawing)
}

#[tracing::instrument(skip_all, fields(trace_id))]
pub(crate) async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let bgp_advertisements = Api::<BGPAdvertisement>::all(client.clone());
    if let Err(e) = bgp_advertisements
        .list(&ListParams::default().limit(1))
        .await
    {
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
            error_policy::<BGPAdvertisement, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
