use std::{
    collections::BTreeMap,
    net::{Ipv4Addr, Ipv6Addr},
    str::FromStr,
    sync::{Arc, Mutex},
};

use futures::StreamExt;
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use kube::{
    api::{ListParams, ObjectMeta, Patch, PatchParams, PostParams},
    runtime::{
        controller::Action,
        finalizer::{finalizer, Event},
        watcher::Config,
        Controller,
    },
    Api, Client, ResourceExt,
};
use sartd_ipam::manager::BlockAllocator;

use crate::{
    controller::{
        context::{error_policy, ContextWith, Ctx, State},
        error::Error,
        metrics::Metrics,
    },
    crd::{
        address_block::{AddressBlock, AddressBlockSpec, AddressBlockStatus},
        address_pool::{AddressPool, AddressType, ADDRESS_POOL_ANNOTATION},
        block_request::{BlockRequest, BLOCK_REQUEST_FINALIZER},
    },
};

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(
    br: Arc<BlockRequest>,
    ctx: Arc<ContextWith<Arc<BlockAllocator>>>,
) -> Result<Action, Error> {
    let block_request_api = Api::<BlockRequest>::all(ctx.client().clone());
    let metrics = ctx.inner.metrics();
    metrics
        .lock()
        .map_err(|_| Error::FailedToGetLock)?
        .reconciliation(br.as_ref());

    finalizer(
        &block_request_api,
        BLOCK_REQUEST_FINALIZER,
        br,
        |event| async {
            match event {
                Event::Apply(br) => reconcile(&block_request_api, &br, ctx.clone()).await,
                Event::Cleanup(br) => cleanup(&block_request_api, &br, ctx.clone()).await,
            }
        },
    )
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all)]
async fn reconcile(
    _api: &Api<BlockRequest>,
    br: &BlockRequest,
    ctx: Arc<ContextWith<Arc<BlockAllocator>>>,
) -> Result<Action, Error> {
    tracing::info!(name = br.name_any(), "Reconcile BlockRequest");

    let address_pool_api = Api::<AddressPool>::all(ctx.client().clone());
    let address_block_api = Api::<AddressBlock>::all(ctx.client().clone());
    let ssapply = PatchParams::apply("agent-blockrequest");

    let ap = address_pool_api
        .get(&br.spec.pool)
        .await
        .map_err(Error::Kube)?;

    if ap.spec.r#type.ne(&AddressType::Pod) {
        return Err(Error::InvalidAddressType);
    }

    if br.spec.pool.ne(&ap.name_any()) {
        tracing::error!(
            name = ap.name_any(),
            target = br.spec.pool,
            "Request is not for this pool"
        );
    }

    let ap_cidr = IpNet::from_str(&ap.spec.cidr).map_err(|_| Error::InvalidCIDR)?;

    let block_prefix_len = ap.spec.block_size.unwrap_or(ap_cidr.prefix_len() as u32);
    let block_size = 2 ^ (block_prefix_len - ap_cidr.prefix_len() as u32);
    let (block_index, ab_name) = {
        let tmp = ctx.component.clone();
        let mut block_allocator = tmp.inner.lock().map_err(|_| Error::FailedToGetLock)?;
        if let Some(pool) = block_allocator.get_mut(&ap.name_any()) {
            let count = pool.counter;
            let ab_name = format!("{}-{count}", ap.name_any());
            let block_index = pool.allocate(&ab_name).map_err(Error::Ipam)?;
            (block_index, ab_name)
        } else {
            return Err(Error::InvalidPool);
        }
    };
    let block_cidr = match get_block_cidr(
        &ap_cidr,
        block_prefix_len,
        block_index * (block_size + 1) as u128,
    ) {
        Ok(cidr) => cidr,
        Err(e) => {
            tracing::error!(name=ap.name_any(), request=br.name_any(), error=?e, "Failed to get block CIDR");
            return Err(e);
        }
    };
    tracing::info!(
        name = ap.name_any(),
        block = ab_name,
        request = br.name_any(),
        index = block_index,
        "Create new AddressBlock by a request"
    );
    let ab = AddressBlock {
        metadata: ObjectMeta {
            labels: Some(BTreeMap::from([(
                ADDRESS_POOL_ANNOTATION.to_string(),
                ap.name_any(),
            )])),
            name: Some(ab_name.clone()),
            ..Default::default()
        },
        spec: AddressBlockSpec {
            cidr: block_cidr.to_string(),
            r#type: AddressType::Pod,
            pool_ref: ap.name_any(),
            node_ref: Some(br.spec.node.clone()),
            auto_assign: ap.spec.auto_assign.unwrap_or(false),
        },
        status: Some(AddressBlockStatus {
            index: block_index as u32,
        }),
    };

    match address_block_api
        .get_opt(&ab.name_any())
        .await
        .map_err(Error::Kube)?
    {
        Some(_) => {
            // already exists
        }
        None => {
            if let Err(e) = address_block_api.create(&PostParams::default(), &ab).await {
                tracing::error!(name=ap.name_any(), block=ab_name, request=br.name_any(), error=?e, "Failed to create AddressBlock");
                return Err(Error::Kube(e));
            }
            let ab_patch = Patch::Merge(&ab);
            address_block_api
                .patch_status(&ab.name_any(), &ssapply, &ab_patch)
                .await
                .map_err(Error::Kube)?;

            ctx.metrics()
                .lock()
                .map_err(|_| Error::FailedToGetLock)?
                .allocated_blocks_inc(&ab.spec.pool_ref, &format!("{}", ab.spec.r#type));
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup(
    _api: &Api<BlockRequest>,
    br: &BlockRequest,
    _ctx: Arc<ContextWith<Arc<BlockAllocator>>>,
) -> Result<Action, Error> {
    tracing::info!(name = br.name_any(), "clean up BlockRequest");
    Ok(Action::await_change())
}

pub async fn run(
    state: State,
    interval: u64,
    block_allocator: Arc<BlockAllocator>,
    metrics: Arc<Mutex<Metrics>>,
) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let block_request_api = Api::<BlockRequest>::all(client.clone());
    if let Err(e) = block_request_api
        .list(&ListParams::default().limit(1))
        .await
    {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start BlockRequest reconciler");

    Controller::new(block_request_api, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<BlockRequest, Error, ContextWith<Arc<BlockAllocator>>>,
            state.to_context_with(client, interval, block_allocator, metrics),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

fn get_block_cidr(cidr: &IpNet, block_size: u32, head: u128) -> Result<IpNet, Error> {
    match cidr {
        IpNet::V4(cidr) => {
            let start = cidr.network();
            let a = u32::from(cidr.addr());

            let n = u32::from(start) + (head as u32);

            let block_start = Ipv4Addr::from(if a > n { a } else { n });
            Ok(IpNet::V4(
                Ipv4Net::new(block_start, block_size as u8).map_err(|_| Error::InvalidCIDR)?,
            ))
        }
        IpNet::V6(cidr) => {
            let start = cidr.network();
            let a = u128::from(cidr.addr());

            let n = u128::from(start) + head;

            let block_start = Ipv6Addr::from(if a > n { a } else { n });
            Ok(IpNet::V6(
                Ipv6Net::new(block_start, block_size as u8).map_err(|_| Error::InvalidCIDR)?,
            ))
        }
    }
}
