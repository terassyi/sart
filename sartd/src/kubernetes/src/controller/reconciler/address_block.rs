use std::{str::FromStr, sync::Arc};

use futures::StreamExt;
use ipnet::IpNet;
use kube::{
    api::ListParams, runtime::{
        controller::{Action, Controller},
        finalizer::{finalizer, Event},
        watcher::Config,
    }, Api, Client, ResourceExt
};

use sartd_ipam::manager::{AllocatorSet, Block, BlockAllocator};

use crate::{
    context::{error_policy, ContextWith, Ctx, State},
    controller::error::Error,
    crd::{address_block::{AddressBlock, ADDRESS_BLOCK_FINALIZER_CONTROLLER}, address_pool::AddressType},
};

#[derive(Debug, Clone)]
pub struct ControllerAddressBlockContext {
    pub allocator_set: Arc<AllocatorSet>,
    pub block_allocator: Arc<BlockAllocator>,
}

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(
    ab: Arc<AddressBlock>,
    ctx: Arc<ContextWith<ControllerAddressBlockContext>>,
) -> Result<Action, Error> {
    let address_blocks = Api::<AddressBlock>::all(ctx.client().clone());

    finalizer(
        &address_blocks,
        ADDRESS_BLOCK_FINALIZER_CONTROLLER,
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
    ctx: Arc<ContextWith<ControllerAddressBlockContext>>,
) -> Result<Action, Error> {
    match ab.spec.r#type {
        AddressType::Pod => reconcile_pod(api, ab, ctx).await,
        AddressType::Service => reconcile_service(api, ab, ctx).await
    }
}

#[tracing::instrument(skip_all)]
async fn reconcile_pod(
    _api: &Api<AddressBlock>,
    _ab: &AddressBlock,
    _ctx: Arc<ContextWith<ControllerAddressBlockContext>>,
) -> Result<Action, Error> {
    Ok(Action::await_change())
}


#[tracing::instrument(skip_all)]
async fn reconcile_service(
    _api: &Api<AddressBlock>,
    ab: &AddressBlock,
    ctx: Arc<ContextWith<ControllerAddressBlockContext>>,
) -> Result<Action, Error> {
    // only handling lb address block here
    if ab.spec.r#type.ne(&AddressType::Service) {
        return Ok(Action::await_change());
    }
    tracing::info!(name = ab.name_any(), "Reconcile AddressBlock");

    let component = ctx.component.allocator_set.clone();
    let mut alloc_set = component.inner.lock().map_err(|_| Error::FailedToGetLock)?;

    let cidr = IpNet::from_str(&ab.spec.cidr).map_err(|_| Error::InvalidCIDR)?;

    match alloc_set.blocks.get(&ab.name_any()) {
        Some(_a) => {
            match ab.spec.auto_assign {
                true => {
                    // Check if already set
                    match &alloc_set.auto_assign {
                        Some(name) => {
                            if ab.name_any().ne(name) {
                                tracing::warn!(
                                    name = ab.name_any(),
                                    "Auto assignable block already exists."
                                );
                                return Err(Error::AutoAssignMustBeOne);
                            }
                        }
                        None => {
                            alloc_set.auto_assign = Some(ab.name_any());
                            tracing::info!(name = ab.name_any(), "Enable auto assign.");
                        }
                    }
                }
                false => {
                    if let Some(name) = &alloc_set.auto_assign {
                        if ab.name_any().eq(name) {
                            tracing::info!(name = ab.name_any(), "Disable auto assign.");
                            alloc_set.auto_assign = None;
                        }
                    }
                }
            }
            if let Some(auto_assign_name) = &alloc_set.auto_assign {
                // If disable auto assign
                if !ab.spec.auto_assign && auto_assign_name.eq(&ab.name_any()) {
                    tracing::info!(name = ab.name_any(), "Disable auto assign");
                }
            }
        }
        None => {
            let block = Block::new(ab.name_any(), ab.name_any(), cidr).map_err(Error::Ipam)?;
            alloc_set.blocks.insert(ab.name_any(), block);
            if ab.spec.auto_assign {
                match &alloc_set.auto_assign {
                    Some(_a) => {
                        tracing::warn!(name = ab.name_any(), "Cannot override auto assign.");
                        return Err(Error::FailedToEnableAutoAssign);
                    }
                    None => {
                        alloc_set.auto_assign = Some(ab.name_any());
                    }
                }
            }
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup(
    api: &Api<AddressBlock>,
    ab: &AddressBlock,
    ctx: Arc<ContextWith<ControllerAddressBlockContext>>,
) -> Result<Action, Error> {
    match ab.spec.r#type {
        AddressType::Pod => cleanup_pod(api, ab, ctx).await,
        AddressType::Service => cleanup_service(api, ab, ctx).await,
    }
}

#[tracing::instrument(skip_all)]
async fn cleanup_pod(
    _api: &Api<AddressBlock>,
    ab: &AddressBlock,
    ctx: Arc<ContextWith<ControllerAddressBlockContext>>,
) -> Result<Action, Error> {

    let index = match &ab.status {
        Some(status) => status.index,
        None => return Err(Error::AddressBlockIndexNotSet)
    };

    {
        let tmp = ctx.component.block_allocator.clone();
        let mut block_allocator = tmp.inner.lock().map_err(|_| Error::FailedToGetLock)?;
        if let Some(pool) = block_allocator.get_mut(&ab.spec.pool_ref) {
            tracing::info!(pool=ab.spec.pool_ref, cidr=?pool.cidr, block_size=pool.block_size,"Remove the pool from the block allocator");
            pool.release(index).map_err(Error::Ipam)?;
        }
    }
    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup_service(
    _api: &Api<AddressBlock>,
    ab: &AddressBlock,
    ctx: Arc<ContextWith<ControllerAddressBlockContext>>,
) -> Result<Action, Error> {
    tracing::info!(name = ab.name_any(), "clean up AddressBlock");

    let component = ctx.component.allocator_set.clone();
    let mut alloc_set = component.inner.lock().map_err(|_| Error::FailedToGetLock)?;

    if let Some(auto) = alloc_set.auto_assign.as_ref() {
        if ab.name_any().eq(auto) {
            tracing::warn!(
                name = ab.name_any(),
                "auto assignable block is not deletable"
            );
            return Err(Error::CannotDelete);
        }
    }

    let mut deletable = false;
    if let Some(block) = alloc_set.get(&ab.name_any()) {
        if !block.allocator.is_empty() {
            return Err(Error::NotEmpty);
        }

        deletable = true;
    }

    if deletable {
        tracing::warn!(name = ab.name_any(), "delete block");
        alloc_set.remove(&ab.name_any());
    }

    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64, ctx: ControllerAddressBlockContext) {
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
            error_policy::<AddressBlock, Error, ContextWith<ControllerAddressBlockContext>>,
            state.to_context_with::<ControllerAddressBlockContext>(client, interval, ctx),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use ipnet::IpNet;
    use kube::{core::ObjectMeta, Api};
    use sartd_ipam::manager::{AllocatorSet, Block, BlockAllocator};

    use crate::{
        context::{ContextWith, Ctx},
        controller::{error::Error, reconciler::address_block::ControllerAddressBlockContext},
        crd::address_block::{AddressBlock, AddressBlockSpec},
        fixture::reconciler::{timeout_after_1s, ApiServerVerifier},
    };

    use super::reconcile;

    enum Scenario {
        Create(AddressBlock),
    }

    impl ApiServerVerifier {
        fn address_block_run(self, scenario: Scenario) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async move {
                match scenario {
                    Scenario::Create(ab) => self.create_address_block(&ab).await,
                }
                .expect("reconcile completed without error");
            })
        }

        async fn create_address_block(self, _ab: &AddressBlock) -> Result<Self, Error> {
            Ok(self)
        }
    }

    #[tokio::test]
    async fn create_address_block() {
        let alloc_set = Arc::new(AllocatorSet::default());
        let block_allocator = Arc::new(BlockAllocator::default());
        let ab_ctx = ControllerAddressBlockContext {
            allocator_set: alloc_set.clone(),
            block_allocator: block_allocator.clone(),
        };
        let (testctx, fakeserver, _) = ContextWith::test(ab_ctx);
        let ab = AddressBlock {
            metadata: ObjectMeta {
                name: Some("test-block".to_string()),
                ..Default::default()
            },
            spec: AddressBlockSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: crate::crd::address_pool::AddressType::Service,
                pool_ref: "test-pool".to_string(),
                node_ref: None,
                auto_assign: false,
            },
            status: None,
        };
        let mocksrv = fakeserver.address_block_run(Scenario::Create(ab.clone()));
        let api = Api::<AddressBlock>::all(testctx.client().clone());
        reconcile(&api, &ab, testctx).await.expect("reconciler");
        timeout_after_1s(mocksrv).await;

        let alloc_set_inner = alloc_set.inner.lock().unwrap();
        let block = alloc_set_inner.get("test-block").unwrap();
        assert_eq!(
            IpNet::from_str("10.0.0.0/16").unwrap(),
            *block.allocator.cidr()
        );
        assert_eq!(None, alloc_set_inner.auto_assign)
    }

    #[tokio::test]
    async fn update_address_block_to_auto_assign() {
        let alloc_set = Arc::new(AllocatorSet::default());
        let block_allocator = Arc::new(BlockAllocator::default());
        let ab_ctx = ControllerAddressBlockContext {
            allocator_set: alloc_set.clone(),
            block_allocator: block_allocator.clone(),
        };
        let (testctx, fakeserver, _) = ContextWith::test(ab_ctx);
        let ab = AddressBlock {
            metadata: ObjectMeta {
                name: Some("test-block".to_string()),
                ..Default::default()
            },
            spec: AddressBlockSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: crate::crd::address_pool::AddressType::Service,
                pool_ref: "test-pool".to_string(),
                node_ref: None,
                auto_assign: true,
            },
            status: None,
        };
        let mocksrv = fakeserver.address_block_run(Scenario::Create(ab.clone()));
        let api = Api::<AddressBlock>::all(testctx.client().clone());
        reconcile(&api, &ab, testctx).await.expect("reconciler");
        timeout_after_1s(mocksrv).await;

        let alloc_set_inner = alloc_set.inner.lock().unwrap();
        assert_eq!(Some("test-block".to_string()), alloc_set_inner.auto_assign)
    }

    #[tokio::test]
    async fn create_address_block_fail_to_override_auto_assign() {
        let alloc_set = Arc::new(AllocatorSet::default());
        {
            let mut alloc_set_inner = alloc_set.inner.lock().unwrap();
            alloc_set_inner
                .insert(
                    Block::new(
                        "other-default-block".to_string(),
                        "other-default-pool".to_string(),
                        IpNet::from_str("10.0.0.0/24").unwrap(),
                    )
                    .unwrap(),
                    true,
                )
                .unwrap();
        }
        {
            let alloc_set_inner = alloc_set.inner.lock().unwrap();
            assert_eq!(
                Some("other-default-block".to_string()),
                alloc_set_inner.auto_assign
            );
        }
        let block_allocator = Arc::new(BlockAllocator::default());
        let ab_ctx = ControllerAddressBlockContext {
            allocator_set: alloc_set.clone(),
            block_allocator: block_allocator.clone(),
        };
        let (testctx, fakeserver, _) = ContextWith::test(ab_ctx);
        let ab = AddressBlock {
            metadata: ObjectMeta {
                name: Some("test-block".to_string()),
                ..Default::default()
            },
            spec: AddressBlockSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: crate::crd::address_pool::AddressType::Service,
                pool_ref: "test-pool".to_string(),
                node_ref: None,
                auto_assign: true,
            },
            status: None,
        };
        let mocksrv = fakeserver.address_block_run(Scenario::Create(ab.clone()));
        let api = Api::<AddressBlock>::all(testctx.client().clone());
        let res = reconcile(&api, &ab, testctx).await;
        timeout_after_1s(mocksrv).await;

        assert_eq!(
            Error::FailedToEnableAutoAssign.to_string(),
            res.unwrap_err().to_string()
        );

        let alloc_set_inner = alloc_set.inner.lock().unwrap();
        assert_eq!(
            Some("other-default-block".to_string()),
            alloc_set_inner.auto_assign
        )
    }

    #[tokio::test]
    async fn update_address_block_fail_to_disable_auto_assign() {
        let alloc_set = Arc::new(AllocatorSet::default());
        {
            let mut alloc_set_inner = alloc_set.inner.lock().unwrap();
            alloc_set_inner
                .insert(
                    Block::new(
                        "other-default-block".to_string(),
                        "default-pool".to_string(),
                        IpNet::from_str("10.0.0.0/16").unwrap(),
                    )
                    .unwrap(),
                    true,
                )
                .unwrap();
            alloc_set_inner
                .insert(
                    Block::new(
                        "test-block".to_string(),
                        "default-pool".to_string(),
                        IpNet::from_str("10.0.0.0/16").unwrap(),
                    )
                    .unwrap(),
                    false,
                )
                .unwrap();
        }
        {
            let alloc_set_inner = alloc_set.inner.lock().unwrap();
            assert_eq!(
                Some("other-default-block".to_string()),
                alloc_set_inner.auto_assign
            );
        }
        let block_allocator = Arc::new(BlockAllocator::default());
        let ab_ctx = ControllerAddressBlockContext {
            allocator_set: alloc_set.clone(),
            block_allocator: block_allocator.clone(),
        };
        let (testctx, fakeserver, _) = ContextWith::test(ab_ctx);
        let ab = AddressBlock {
            metadata: ObjectMeta {
                name: Some("test-block".to_string()),
                ..Default::default()
            },
            spec: AddressBlockSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: crate::crd::address_pool::AddressType::Service,
                pool_ref: "test-pool".to_string(),
                node_ref: None,
                auto_assign: true,
            },
            status: None,
        };
        let mocksrv = fakeserver.address_block_run(Scenario::Create(ab.clone()));
        let api = Api::<AddressBlock>::all(testctx.client().clone());
        let res = reconcile(&api, &ab, testctx).await;
        timeout_after_1s(mocksrv).await;

        assert_eq!(
            Error::AutoAssignMustBeOne.to_string(),
            res.unwrap_err().to_string()
        );

        let alloc_set_inner = alloc_set.inner.lock().unwrap();
        assert_eq!(
            Some("other-default-block".to_string()),
            alloc_set_inner.auto_assign
        );

        // Confirm that the test-block's auto assign field is set as false.
    }
}
