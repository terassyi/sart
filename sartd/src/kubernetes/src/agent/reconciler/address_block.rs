use std::{collections::BTreeMap, str::FromStr, sync::Arc, time::Duration};

use futures::StreamExt;
use ipnet::IpNet;
use kube::{
    api::{DeleteParams, ListParams, PostParams},
    core::ObjectMeta,
    runtime::{
        controller::Action,
        finalizer::{finalizer, Event},
        watcher::Config,
        Controller,
    },
    Api, Client, ResourceExt,
};
use sartd_ipam::manager::{AllocatorSet, Block};
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    agent::{error::Error, reconciler::node_bgp::ENV_HOSTNAME},
    context::{error_policy, ContextWith, Ctx, State},
    crd::{
        address_block::{AddressBlock, ADDRESS_BLOCK_FINALIZER, ADDRESS_BLOCK_NODE_LABEL},
        address_pool::{AddressType, ADDRESS_POOL_ANNOTATION},
        bgp_advertisement::{
            AdvertiseStatus, BGPAdvertisement, BGPAdvertisementSpec, BGPAdvertisementStatus,
            Protocol,
        },
        node_bgp::NodeBGP,
    },
    util::create_owner_reference,
};

#[derive(Debug)]
pub struct PodAllocator {
    pub allocator: Arc<AllocatorSet>,
    pub notifier: UnboundedSender<AddressBlock>,
}

pub async fn reconciler(
    ab: Arc<AddressBlock>,
    ctx: Arc<ContextWith<Arc<PodAllocator>>>,
) -> Result<Action, Error> {
    // handle only Pod type
    if ab.spec.r#type.ne(&AddressType::Pod) {
        return Ok(Action::await_change());
    }
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
    api: &Api<AddressBlock>,
    ab: &AddressBlock,
    ctx: Arc<ContextWith<Arc<PodAllocator>>>,
) -> Result<Action, Error> {
    tracing::info!(name = ab.name_any(), "Reconcile AddressBlock");

    let component = ctx.component.clone();

    let namespace = "kube-system".to_string();
    let cidr = IpNet::from_str(&ab.spec.cidr).map_err(|_| Error::InvalidCIDR)?;
    let pool_name = ab.spec.pool_ref.as_str();
    let node = ab
        .spec
        .node_ref
        .clone()
        .ok_or(Error::MissingFields("spec.node_ref".to_string()))?;

    let adv_api = Api::<BGPAdvertisement>::namespaced(ctx.client().clone(), &namespace);
    let mut create_adv = false;
    let mut need_update_adv = false;
    // let mut need_gc = false;

    if let Some(adv) = adv_api.get_opt(&ab.name_any()).await.map_err(Error::Kube)? {
        match adv.status.as_ref() {
            Some(status) => match status.peers.as_ref() {
                Some(peers) => {
                    if peers.is_empty() {
                        need_update_adv = true;
                    }
                }
                None => need_update_adv = true,
            },
            None => need_update_adv = true,
        }
    } else {
        create_adv = true;
    }

    {
        let mut alloc_set = component
            .allocator
            .inner
            .lock()
            .map_err(|_| Error::FailedToGetLock)?;
        match alloc_set.blocks.get(&ab.name_any()) {
            Some(_block) => {
                tracing::info!(name = ab.name_any(), "Address block already exists");

                // GC empty block
                // if block.allocator.is_empty() {
                //     tracing::info!(name = ab.name_any(), "Block is empty");
                //     need_gc = true;
                // }

                match ab.spec.auto_assign {
                    true => {
                        // Check if already set
                        match &alloc_set.auto_assign {
                            Some(name) => {
                                if name.ne(pool_name) {
                                    tracing::error!(
                                        name = ab.name_any(),
                                        pool = pool_name,
                                        "Auto assignable pool already exists."
                                    );
                                    return Err(Error::AutoAssignAlreadyExists);
                                }
                            }
                            None => {
                                alloc_set.auto_assign = Some(pool_name.to_string());
                                tracing::info!(name = ab.name_any(), "Enable auto assign.");
                            }
                        }
                    }
                    false => {
                        if let Some(name) = &alloc_set.auto_assign {
                            if name.eq(pool_name) {
                                tracing::info!(name = ab.name_any(), "Disable auto assign.");
                                alloc_set.auto_assign = None;
                            }
                        }
                    }
                }
            }
            None => {
                let block =
                    Block::new(ab.name_any(), pool_name.to_string(), cidr).map_err(Error::Ipam)?;
                if ab.spec.auto_assign {
                    match &alloc_set.auto_assign {
                        Some(name) => {
                            if name.ne(pool_name) {
                                tracing::warn!(
                                    name = ab.name_any(),
                                    "Cannot override auto assign."
                                );
                                return Err(Error::AutoAssignAlreadyExists);
                            }
                        }
                        None => {
                            alloc_set.auto_assign = Some(pool_name.to_string());
                        }
                    }
                }
                alloc_set.blocks.insert(ab.name_any(), block);

                create_adv = true;

                tracing::info!(name = ab.name_any(), "Create new allocator block");
                component
                    .notifier
                    .send(ab.clone())
                    .map_err(|_| Error::FailedToNotify)?;
            }
        }
    }

    // if need_gc {
    //     tracing::info!(name = ab.name_any(), "Delete empty AddressBlock");
    //     api.delete(&ab.name_any(), &DeleteParams::default())
    //         .await
    //         .map_err(Error::Kube)?;
    //     return Ok(Action::await_change());
    // }

    if create_adv {
        let adv = BGPAdvertisement {
            metadata: ObjectMeta {
                name: Some(ab.name_any()),
                namespace: Some(namespace.to_string()), // controller's namespace
                labels: Some(BTreeMap::from([(
                    ADDRESS_POOL_ANNOTATION.to_string(),
                    ab.spec.pool_ref.clone(),
                )])),
                owner_references: Some(vec![create_owner_reference(ab)]),
                ..Default::default()
            },
            spec: BGPAdvertisementSpec {
                cidr: cidr.to_string(),
                r#type: AddressType::Pod,
                protocol: Protocol::from(&cidr),
                attrs: None,
            },
            status: None,
        };

        adv_api
            .create(&PostParams::default(), &adv)
            .await
            .map_err(Error::Kube)?;

        return Ok(Action::requeue(Duration::from_secs(1)));
    }

    if need_update_adv {
        let mut adv = adv_api.get(&ab.name_any()).await.map_err(Error::Kube)?;
        let peers = get_target_peer(ctx.client().clone(), &node)
            .await?
            .into_iter()
            .map(|p| (p, AdvertiseStatus::NotAdvertised))
            .collect::<BTreeMap<String, AdvertiseStatus>>();
        match adv.status.as_mut() {
            Some(status) => match status.peers.as_mut() {
                Some(status_peers) => {
                    for (peer, _) in peers.iter() {
                        if status_peers.get(peer).is_none() {
                            status_peers.insert(peer.to_string(), AdvertiseStatus::NotAdvertised);
                        }
                    }
                }
                None => status.peers = Some(peers),
            },
            None => {
                adv.status = Some(BGPAdvertisementStatus {
                    peers: Some(peers.clone()),
                });
            }
        }
        adv_api
            .replace_status(
                &adv.name_any(),
                &PostParams::default(),
                serde_json::to_vec(&adv).map_err(Error::Serialization)?,
            )
            .await
            .map_err(Error::Kube)?;
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup(
    _api: &Api<AddressBlock>,
    ab: &AddressBlock,
    ctx: Arc<ContextWith<Arc<PodAllocator>>>,
) -> Result<Action, Error> {
    tracing::info!(name = ab.name_any(), "clean up AddressBlock");

    let component = ctx.component.clone();
    let namespace = "kube-system".to_string();
    let mut deletable = false;

    {
        let mut alloc_set = component
            .allocator
            .inner
            .lock()
            .map_err(|_| Error::FailedToGetLock)?;

        if let Some(block) = alloc_set.get(&ab.name_any()) {
            if !block.allocator.is_empty() {
                return Err(Error::NotEmpty);
            }

            deletable = true;
        }

        if deletable {
            tracing::warn!(name = ab.name_any(), "Delete the block");
            alloc_set.remove(&ab.name_any());
        }
    }

    if deletable {
        let adv_api = Api::<BGPAdvertisement>::namespaced(ctx.client().clone(), &namespace);
        adv_api
            .delete(&ab.name_any(), &DeleteParams::default())
            .await
            .map_err(Error::Kube)?;
    }
    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64, pod_allocator: Arc<PodAllocator>) {
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
    let node_name = std::env::var(ENV_HOSTNAME).expect("HOSTNAME environment value is not set");
    let label_selector = format!("{}={}", ADDRESS_BLOCK_NODE_LABEL, node_name);
    let watch_config = Config::default().labels(&label_selector);

    Controller::new(address_blocks, watch_config.any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<AddressBlock, Error, ContextWith<Arc<PodAllocator>>>,
            state.to_context_with::<Arc<PodAllocator>>(client, interval, pod_allocator),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

async fn get_target_peer(client: Client, node: &str) -> Result<Vec<String>, Error> {
    let node_bgp_api = Api::<NodeBGP>::all(client);

    let nb = node_bgp_api.get(node).await.map_err(Error::Kube)?;
    match nb.spec.peers.as_ref() {
        Some(peers) => Ok(peers
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<String>>()),
        None => Ok(vec![]),
    }
}
