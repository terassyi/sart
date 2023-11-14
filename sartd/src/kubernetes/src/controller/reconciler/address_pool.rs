use std::sync::Arc;

use futures::StreamExt;
use kube::{
    api::{ListParams, PostParams},
    core::ObjectMeta,
    runtime::{
        controller::{Action, Controller},
        finalizer::{finalizer, Event},
        watcher::Config,
    },
    Api, Client, ResourceExt,
};

use crate::{
    context::{error_policy, Context, State},
    controller::error::Error,
    crd::{
        address_block::{AddressBlock, AddressBlockSpec},
        address_pool::{AddressPool, AddressPoolStatus, AddressType, ADDRESS_POOL_FINALIZER},
    },
    util::create_owner_reference,
};

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(ap: Arc<AddressPool>, ctx: Arc<Context>) -> Result<Action, Error> {
    let address_pools = Api::<AddressPool>::all(ctx.client.clone());

    finalizer(&address_pools, ADDRESS_POOL_FINALIZER, ap, |event| async {
        match event {
            Event::Apply(ap) => reconcile(&address_pools, &ap, ctx.clone()).await,
            Event::Cleanup(ap) => cleanup(&address_pools, &ap, ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all)]
async fn reconcile(
    api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    tracing::info!(name = ap.name_any(), "reconcile AddressPool");

    match ap.spec.r#type {
        AddressType::Service => reconcile_service_pool(api, ap, ctx).await,
    }
}

#[tracing::instrument(skip_all)]
async fn reconcile_service_pool(
    api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    let address_blocks = Api::<AddressBlock>::all(ctx.client.clone());

    match address_blocks
        .get_opt(&ap.name_any())
        .await
        .map_err(Error::Kube)?
    {
        Some(ab) => {
            tracing::warn!(name = ab.name_any(), "AddressBlock already exists");
        }
        None => {
            let ab = AddressBlock {
                metadata: ObjectMeta {
                    name: Some(ap.name_any()),
                    owner_references: Some(vec![create_owner_reference(ap)]),
                    ..Default::default()
                },
                spec: AddressBlockSpec {
                    cidr: ap.spec.cidr.clone(),
                    r#type: ap.spec.r#type,
                    pool_ref: ap.name_any(),
                    node_ref: None,
                    auto_assign: ap.spec.auto_assign.unwrap_or(false),
                },
                status: None,
            };

            address_blocks
                .create(&PostParams::default(), &ab)
                .await
                .map_err(Error::Kube)?;
        }
    }

    let mut new_ap = ap.clone();
    let mut need_update = false;
    match new_ap
        .status
        .as_mut()
        .and_then(|status| status.blocks.as_mut())
    {
        Some(blocks) => {
            if !blocks.contains(&ap.name_any()) {
                blocks.push(ap.name_any());
                need_update = true;
            }
        }
        None => {
            new_ap.status = Some(AddressPoolStatus {
                blocks: Some(vec![ap.name_any()]),
            });
            need_update = true;
        }
    }

    if need_update {
        api.replace_status(
            &ap.name_any(),
            &PostParams::default(),
            serde_json::to_vec(&new_ap).map_err(Error::Serialization)?,
        )
        .await
        .map_err(Error::Kube)?;
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup(
    api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let address_pools = Api::<AddressPool>::all(client.clone());
    if let Err(e) = address_pools.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start AddressPool reconciler");

    Controller::new(address_pools, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<AddressPool, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
