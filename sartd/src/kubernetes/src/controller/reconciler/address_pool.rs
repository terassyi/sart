use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
    sync::Arc,
};

use futures::StreamExt;

use ipnet::{IpNet, Ipv4Net, Ipv6Net};

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
use sartd_ipam::manager::{BlockAllocator, Pool};
use tracing::{field, Span};

use crate::{
    context::{error_policy, ContextWith, Ctx, State},
    controller::error::Error,
    crd::{
        address_block::{AddressBlock, AddressBlockSpec},
        address_pool::{AddressPool, AddressType, ADDRESS_POOL_ANNOTATION, ADDRESS_POOL_FINALIZER},
    },
    util::create_owner_reference,
};

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(
    ap: Arc<AddressPool>,
    ctx: Arc<ContextWith<Arc<BlockAllocator>>>,
) -> Result<Action, Error> {
    let address_pools = Api::<AddressPool>::all(ctx.client().clone());

    finalizer(&address_pools, ADDRESS_POOL_FINALIZER, ap, |event| async {
        match event {
            Event::Apply(ap) => reconcile(&address_pools, &ap, ctx.clone()).await,
            Event::Cleanup(ap) => cleanup(&address_pools, &ap, ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(
    api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<ContextWith<Arc<BlockAllocator>>>,
) -> Result<Action, Error> {
    let trace_id = sartd_trace::telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));

    match ap.spec.r#type {
        AddressType::Service => reconcile_service_pool(api, ap, ctx).await,
        AddressType::Pod => reconcile_pod_pool(api, ap, ctx).await,
    }
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile_service_pool(
    api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<ContextWith<Arc<BlockAllocator>>>,
) -> Result<Action, Error> {
    let address_blocks = Api::<AddressBlock>::all(ctx.client().clone());

    let cidr = IpNet::from_str(&ap.spec.cidr).map_err(|_| Error::InvalidCIDR)?;
    let block_size = ap.spec.block_size.unwrap_or(cidr.prefix_len() as u32);

    match address_blocks
        .get_opt(&ap.name_any())
        .await
        .map_err(Error::Kube)?
    {
        Some(ab) => {
            tracing::warn!(name = ab.name_any(), "address block already exists");
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

            let pool = Pool::new(&cidr, block_size);

            {
                let tmp = ctx.component.clone();
                let mut block_allocator = tmp.inner.lock().map_err(|_| Error::FailedToGetLock)?;
                if block_allocator.get(&ap.name_any()).is_none() {
                    tracing::info!(
                        name = ap.name_any(),
                        "Insert address pool into block allocator"
                    );
                    block_allocator.insert(&ap.name_any(), pool);
                }
            }
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn reconcile_pod_pool(
    api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<ContextWith<Arc<BlockAllocator>>>,
) -> Result<Action, Error> {
    tracing::info!(name = ap.name_any(), "Reconcile AddressPool");
    let cidr = IpNet::from_str(&ap.spec.cidr).map_err(|_| Error::InvalidCIDR)?;
    let block_size = ap.spec.block_size.unwrap_or(cidr.prefix_len() as u32);
    {
        let tmp = ctx.component.clone();
        let mut block_allocator = tmp.inner.lock().map_err(|_| Error::FailedToGetLock)?;
        if block_allocator.get(&ap.name_any()).is_none() {
            let pool = Pool::new(&cidr, block_size);
            tracing::info!(pool=ap.name_any(), cidr=?cidr, block_size=block_size,"Insert a new pool into the block allocator");
            block_allocator.insert(&ap.name_any(), pool)
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(
    _api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<ContextWith<Arc<BlockAllocator>>>,
) -> Result<Action, Error> {
    tracing::info!(name = ap.name_any(), "Clean up AddressPool");
    let address_block_api = Api::<AddressBlock>::all(ctx.client().clone());
    let list_params =
        ListParams::default().labels(&format!("{}={}", ADDRESS_POOL_ANNOTATION, ap.name_any()));
    let ab_list = address_block_api
        .list(&list_params)
        .await
        .map_err(Error::Kube)?;
    if ab_list.items.len() != 0 {
        return Err(Error::AddressPoolNotEmpty);
    }

    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64, block_allocator: Arc<BlockAllocator>) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let address_pool_api = Api::<AddressPool>::all(client.clone());
    if let Err(e) = address_pool_api.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start AddressPool reconciler");

    Controller::new(address_pool_api, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<AddressPool, Error, ContextWith<Arc<BlockAllocator>>>,
            state.to_context_with(client, interval, block_allocator),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

fn contains_cidr(base: &IpNet, target: &IpNet) -> Result<u128, Error> {
    if !base.contains(target) {
        return Err(Error::InvalidCIDR);
    }

    let base_min = base.addr();
    let target_min = target.addr();

    let sub = match base_min {
        IpAddr::V4(base_min) => {
            let target_v4_min = match target_min {
                IpAddr::V4(t) => t,
                _ => return Err(Error::InvalidProtocol),
            };
            (u32::from(target_v4_min) - u32::from(base_min)) as u128
        }
        IpAddr::V6(base_min) => {
            let target_v6_min = match target_min {
                IpAddr::V6(t) => t,
                _ => return Err(Error::InvalidProtocol),
            };
            u128::from(target_v6_min) - u128::from(base_min)
        }
    };
    Ok(sub)
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest(
        base,
        target,
        expected,
        case(
            IpNet::from_str("10.0.0.0/24").unwrap(),
            IpNet::from_str("10.0.0.0/24").unwrap(),
            0,
        ),
        case(
            IpNet::from_str("10.0.0.0/24").unwrap(),
            IpNet::from_str("10.0.0.0/28").unwrap(),
            0,
        ),
        case(
            IpNet::from_str("10.0.0.0/24").unwrap(),
            IpNet::from_str("10.0.0.16/28").unwrap(),
            16,
        ),
        case(
            IpNet::from_str("10.0.0.0/24").unwrap(),
            IpNet::from_str("10.0.0.144/28").unwrap(),
            144,
        ),
        case(
            IpNet::from_str("10.0.0.0/24").unwrap(),
            IpNet::from_str("10.0.0.8/29").unwrap(),
            8,
        ),
        case(
            IpNet::from_str("192.168.2.4/30").unwrap(),
            IpNet::from_str("192.168.2.6/31").unwrap(),
            2,
        ),
        case(
            IpNet::from_str("fd00::/16").unwrap(),
            IpNet::from_str("fd00::8000/17").unwrap(),
            32768,
        ),
    )]
    fn test_contains_cidr(base: IpNet, target: IpNet, expected: u128) {
        let result = contains_cidr(&base, &target).unwrap();
        assert_eq!(expected, result);
    }

    #[rstest(
        cidr,
        block_size,
        head,
        expected,
        case(
            IpNet::from_str("10.0.0.0/24").unwrap(),
            29,
            8,
            IpNet::from_str("10.0.0.8/29").unwrap(),
        ),
        case(
            IpNet::from_str("10.0.0.0/24").unwrap(),
            28,
            144,
            IpNet::from_str("10.0.0.144/28").unwrap(),
        ),
        case(
            IpNet::from_str("192.168.2.4/30").unwrap(),
            31,
            2,
            IpNet::from_str("192.168.2.6/31").unwrap(),
        ),
        case(
            IpNet::from_str("192.168.2.5/30").unwrap(),
            31,
            0,
            IpNet::from_str("192.168.2.5/31").unwrap(),
        ),
        case(
            IpNet::from_str("fd00::/16").unwrap(),
            17,
            32768,
            IpNet::from_str("fd00::8000/17").unwrap(),
        ),
    )]
    fn test_get_block_cidr(cidr: IpNet, block_size: u32, head: u128, expected: IpNet) {
        let result = get_block_cidr(&cidr, block_size, head).unwrap();
        assert_eq!(expected, result);
    }
}
