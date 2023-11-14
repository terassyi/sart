use std::{str::FromStr, sync::Arc};

use futures::StreamExt;
use ipnet::IpNet;
use kube::{
    api::ListParams,
    runtime::{
        controller::{Action, Controller},
        finalizer::{finalizer, Event},
        watcher::Config,
    },
    Api, Client, ResourceExt,
};

use sartd_ipam::manager::{AllocatorSet, Block};

use crate::{
    context::{error_policy, ContextWith, Ctx, State},
    controller::error::Error,
    crd::address_block::{AddressBlock, ADDRESS_BLOCK_FINALIZER},
};

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(
    ab: Arc<AddressBlock>,
    ctx: Arc<ContextWith<Arc<AllocatorSet>>>,
) -> Result<Action, Error> {
    let address_blocks = Api::<AddressBlock>::all(ctx.client().clone());

    finalizer(
        &address_blocks,
        ADDRESS_BLOCK_FINALIZER,
        ab,
        |event| async {
            match event {
                Event::Apply(ab) => reconcile(&address_blocks, &ab, ctx.clone()).await,
                Event::Cleanup(ab) => cleanup(&address_blocks, &ab, ctx.clone()).await,
            }
        },
    )
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all)]
async fn reconcile(
    _api: &Api<AddressBlock>,
    ab: &AddressBlock,
    ctx: Arc<ContextWith<Arc<AllocatorSet>>>,
) -> Result<Action, Error> {
    tracing::info!(name = ab.name_any(), "reconcile AddressBlock");

    let component = ctx.component.clone();
    let mut alloc_set = component.inner.lock().unwrap();

    let cidr = match IpNet::from_str(&ab.spec.cidr) {
        Ok(c) => c,
        Err(_e) => return Err(Error::InvalidAddress),
    };

    match alloc_set.blocks.get(&ab.name_any()) {
        Some(_) => {
            tracing::info!(name = ab.name_any(), "Address block already exists");
        }
        None => {
            let block = Block::new(ab.name_any(), ab.name_any(), cidr).map_err(Error::Ipam)?;
            alloc_set.blocks.insert(ab.name_any(), block);
            if ab.spec.auto_assign && !alloc_set.auto_assigns.contains(&ab.name_any()) {
                alloc_set.auto_assigns.push(ab.name_any());
            }
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup(
    api: &Api<AddressBlock>,
    ab: &AddressBlock,
    ctx: Arc<ContextWith<Arc<AllocatorSet>>>,
) -> Result<Action, Error> {
    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64, allocator_set: Arc<AllocatorSet>) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let address_blocks = Api::<AddressBlock>::all(client.clone());
    if let Err(e) = address_blocks.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start AddressBlock reconciler");

    Controller::new(address_blocks, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<AddressBlock, Error, ContextWith<Arc<AllocatorSet>>>,
            state.to_context_with::<Arc<AllocatorSet>>(client, interval, allocator_set),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
