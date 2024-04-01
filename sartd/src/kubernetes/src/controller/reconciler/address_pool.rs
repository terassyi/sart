use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use futures::StreamExt;

use ipnet::{AddrParseError, IpNet, Ipv4Net, Ipv6Net};
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{DeleteParams, ListParams, PostParams},
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
        address_pool::{
            AddressPool, AddressPoolStatus, AddressType, ADDRESS_POOL_ANNOTATION,
            ADDRESS_POOL_FINALIZER,
        },
        block_request::BlockRequest,
    },
    util::{create_owner_reference, diff},
};

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(ap: Arc<AddressPool>, ctx: Arc<Context>) -> Result<Action, Error> {
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
        AddressType::Pod => reconcile_pod_pool(api, ap, ctx).await,
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
        .and_then(|status| status.allocated.as_mut())
    {
        Some(allocated) => {
            if allocated.get(&ap.name_any()).is_some() {
                allocated.insert(ap.name_any(), 0);
                need_update = true;
            }
        }
        None => {
            new_ap.status = Some(AddressPoolStatus {
                requested: None,
                allocated: Some(HashMap::from([(ap.name_any(), 0)])),
                released: None,
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
async fn reconcile_pod_pool(
    api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    let cidr: IpNet = ap
        .spec
        .cidr
        .parse()
        .map_err(|e: AddrParseError| Error::InvalidParameter(e.to_string()))?;
    let block_prefix_len = ap.spec.block_size.unwrap_or(cidr.prefix_len() as u32);
    let block_size = 1 << (cidr.max_prefix_len() as u32 - block_prefix_len);
    let pool_size: u128 = 1 << (cidr.max_prefix_len() - cidr.prefix_len());
    tracing::info!(
        block = block_size,
        pool = pool_size,
        max_prefix_len = cidr.max_prefix_len(),
        prefix_len = cidr.prefix_len()
    );

    let mut new_ap = ap.clone();

    let address_block_api = Api::<AddressBlock>::all(ctx.client.clone());
    let params =
        ListParams::default().labels(&format!("{}={}", ADDRESS_POOL_ANNOTATION, ap.name_any()));
    let ab_list = address_block_api.list(&params).await.map_err(Error::Kube)?;
    let ab_cidr_map: HashMap<String, String> = ab_list
        .iter()
        .map(|ab| (ab.name_any(), ab.spec.cidr.clone()))
        .collect();

    let actual_allocated_blocks: Vec<String> = ab_list.iter().map(|ab| ab.name_any()).collect();

    let allocated_blocks = if let Some(allocated) = ap
        .status
        .as_ref()
        .and_then(|status| status.allocated.as_ref())
    {
        let mut allocated_blocks: Vec<String> = allocated.keys().cloned().collect();
        allocated_blocks.sort();
        allocated_blocks
    } else {
        Vec::new()
    };

    let (added, _remain, removed) = diff::<String>(&allocated_blocks, &actual_allocated_blocks);

    let mut need_update = !removed.is_empty() || !added.is_empty();

    for added_ab in added.iter() {
        let block_cidr = match ab_cidr_map.get(added_ab) {
            Some(cidr) => IpNet::from_str(cidr).map_err(|_e| Error::InvalidAddress)?,
            None => {
                return Err(Error::FailedToGetData(
                    "Allocated block is not found".to_string(),
                ))
            }
        };
        let head = contains_cidr(&cidr, &block_cidr)?;
        if head % block_size != 0 {
            tracing::error!(
                name = ap.name_any(),
                block = added_ab,
                block_size = block_size,
                "block's CIDR is invalid"
            );
            continue;
        }
        match new_ap.status.as_mut() {
            Some(status) => match status.allocated.as_mut() {
                Some(allocated) => {
                    allocated.insert(added_ab.to_string(), head as u128);
                }
                None => {
                    status.allocated = Some(HashMap::from([(added_ab.to_string(), head as u128)]))
                }
            },
            None => {
                new_ap.status = Some(AddressPoolStatus {
                    requested: None,
                    allocated: Some(HashMap::from([(added_ab.to_string(), head as u128)])),
                    released: None,
                })
            }
        }
    }

    for removed_ab in removed.iter() {
        match new_ap.status.as_mut() {
            Some(status) => match status.released.as_mut() {
                Some(released) => {
                    if !released.iter().any(|r| r.eq(removed_ab)) {
                        released.push(removed_ab.to_string());
                    }
                }
                None => {
                    status.released = Some(vec![removed_ab.to_string()]);
                }
            },
            None => {
                new_ap.status = Some(AddressPoolStatus {
                    requested: None,
                    allocated: None,
                    released: Some(vec![removed_ab.to_string()]),
                });
            }
        }
    }

    let mut need_requeue = false;

    let mut new_released = Vec::new();

    if let Some(released) = new_ap
        .status
        .as_mut()
        .and_then(|status| status.released.as_mut())
    {
        for r in released.iter() {
            if ab_cidr_map.get(r).is_some() {
                // existing
                new_released.push(r.to_string());
                tracing::info!(name = ap.name_any(), block = r, "Delete the unused block");
                address_block_api
                    .delete(r, &DeleteParams::default())
                    .await
                    .map_err(Error::Kube)?;
                need_requeue = true;
            }
        }
        *released = new_released.clone();
        need_update = true;
    }

    for rel in new_released.iter() {
        // if new_released is not empty, need_update must be true.
        if let Some(allocated) = new_ap
            .status
            .as_mut()
            .and_then(|status| status.allocated.as_mut())
        {
            allocated.remove(rel);
        }
    }

    let all_blocks: Vec<u128> = (0..pool_size).step_by(block_size as usize).collect();
    let allocated_blocks = if let Some(allocated) = ap
        .status
        .as_ref()
        .and_then(|status| status.allocated.as_ref())
    {
        let mut allocated_blocks: Vec<u128> = allocated.values().copied().collect();
        allocated_blocks.sort();
        allocated_blocks
    } else {
        Vec::new()
    };
    let (allocatable_blocks, _, _) = diff::<u128>(&allocated_blocks, &all_blocks);
    if allocatable_blocks.is_empty() {
        tracing::warn!(name=ap.name_any(), allocated=?allocatable_blocks, all=?all_blocks, "AddressPool cannot create new AddressBlock any more");
        return Err(Error::NoAllocatableBlock);
    }

    let mut allocatable_blocks = VecDeque::from(allocatable_blocks);

    let mut new_allocated = Vec::new();
    let mut new_requested = Vec::new();

    let block_request_api = Api::<BlockRequest>::all(ctx.client.clone());
    let node_api = Api::<Node>::all(ctx.client.clone());

    if let Some(requested) = new_ap
        .status
        .as_ref()
        .and_then(|status| status.requested.as_ref())
    {
        if !requested.is_empty() {
            need_update = true;
        }
        for req in requested.iter() {
            let br = match block_request_api.get(req).await.map_err(Error::Kube) {
                Ok(br) => br,
                Err(e) => {
                    tracing::error!(name=ap.name_any(), request=req, error=?e, "BlockRequest doesn't exist");
                    continue;
                }
            };

            if br.spec.pool.ne(&ap.name_any()) {
                tracing::error!(
                    name = ap.name_any(),
                    target = br.spec.pool,
                    "request is not for this pool"
                );
                continue;
            }
            let _node = node_api.get(&br.spec.node).await.map_err(Error::Kube)?;

            let head = match allocatable_blocks.pop_front() {
                Some(head) => head,
                None => {
                    tracing::error!(
                        name = ap.name_any(),
                        target = br.spec.pool,
                        "Requested node doesn't exist"
                    );
                    continue;
                }
            };

            let block_prefix_len = ap.spec.block_size.unwrap_or(cidr.prefix_len() as u32);
            let block_cidr = match get_block_cidr(&cidr, block_prefix_len, head) {
                Ok(cidr) => cidr,
                Err(e) => {
                    tracing::error!(name=ap.name_any(), request=req, error=?e, "Failed to get block CIDR");
                    new_requested.push(req.to_string());
                    continue;
                }
            };
            let ab_name = format!("{}-{}", req, block_cidr.addr());
            tracing::info!(
                name = ap.name_any(),
                block = ab_name,
                request = req,
                head = head,
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
                status: None,
            };

            if let Err(e) = address_block_api
                .create(&PostParams::default(), &ab)
                .await
                .map_err(Error::Kube)
            {
                tracing::error!(name=ap.name_any(), block=ab_name, request=req, error=?e, "Failed to create AddressBlock");
                new_requested.push(req.to_string());
                continue;
            }
            new_allocated.push((ab_name, head))
        }
    }

    match new_ap.status.as_mut() {
        Some(status) => {
            match status.allocated.as_mut() {
                Some(allocated) => {
                    for (k, v) in new_allocated.into_iter() {
                        allocated.insert(k, v);
                    }
                }
                None => {
                    status.allocated = Some(new_allocated.into_iter().collect());
                }
            }
            status.requested = Some(new_requested);
        }
        None => {
            new_ap.status = Some(AddressPoolStatus {
                requested: Some(new_requested),
                allocated: Some(new_allocated.into_iter().collect()),
                released: None,
            });
        }
    }

    if need_update {
        tracing::info!(name=ap.name_any(), status=?new_ap.status, "Update status");
        api.replace_status(
            &ap.name_any(),
            &PostParams::default(),
            serde_json::to_vec(&new_ap).map_err(Error::Serialization)?,
        )
        .await
        .map_err(Error::Kube)?;
    }

    // check auto assign
    let auto_assign = ap.spec.auto_assign.unwrap_or(false);

    if auto_assign {
        tracing::info!(name = ap.name_any(), "Auto assign is enabled");
        let ap_list = api
            .list(&ListParams::default())
            .await
            .map_err(Error::Kube)?;
        let is_exist = ap_list
            .iter()
            .filter(|p| p.spec.r#type.eq(&AddressType::Pod) && p.name_any().ne(&ap.name_any()))
            .fold(false, |other, p| {
                p.spec.auto_assign.unwrap_or(false) || other
            });
        if is_exist {
            tracing::error!(
                name = ap.name_any(),
                "Auto assignable AddressPool for Pod already exists"
            );
            return Err(Error::AutoAssignMustBeOne);
        }
    }

    let ab_list = address_block_api.list(&params).await.map_err(Error::Kube)?;
    for ab in ab_list.iter() {
        if ab.spec.auto_assign != auto_assign {
            let mut new_ab = ab.clone();
            new_ab.spec.auto_assign = auto_assign;
            tracing::info!(
                name = ap.name_any(),
                block = ab.name_any(),
                auto_assign = auto_assign,
                "Change auto assign flag for AddressBlock"
            );
            address_block_api
                .replace(&new_ab.name_any(), &PostParams::default(), &new_ab)
                .await
                .map_err(Error::Kube)?;
        }
    }

    if need_requeue {
        return Ok(Action::requeue(Duration::from_secs(10)));
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup(
    _api: &Api<AddressPool>,
    _ap: &AddressPool,
    _ctx: Arc<Context>,
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
