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
pub async fn reconciler(
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
    let mut alloc_set = component.inner.lock().map_err(|_| Error::FailedToGetLock)?;

	let cidr = IpNet::from_str(&ab.spec.cidr).map_err(|_| Error::InvalidCIDR)?;

    match alloc_set.blocks.get(&ab.name_any()) {
        Some(_a) => {
            tracing::info!(name = ab.name_any(), "Address block already exists");
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
    _api: &Api<AddressBlock>,
    ab: &AddressBlock,
    ctx: Arc<ContextWith<Arc<AllocatorSet>>>,
) -> Result<Action, Error> {
    tracing::info!(name = ab.name_any(), "clean up AddressBlock");

    let component = ctx.component.clone();
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

#[cfg(test)]
mod tests {
    use std::{str::FromStr, sync::Arc};

    use ipnet::IpNet;
    use kube::{core::ObjectMeta, Api};
    use sartd_ipam::manager::{AllocatorSet, Block};

    use crate::{
        context::{ContextWith, Ctx},
        controller::error::Error,
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
        let (testctx, fakeserver, _) = ContextWith::test(alloc_set.clone());
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
        let (testctx, fakeserver, _) = ContextWith::test(alloc_set.clone());
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
        let (testctx, fakeserver, _) = ContextWith::test(alloc_set.clone());
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
        let (testctx, fakeserver, _) = ContextWith::test(alloc_set.clone());
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
