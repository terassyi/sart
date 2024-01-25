use std::{net::IpAddr, str::FromStr, sync::Arc};

use kube::{
    api::{DeleteParams, Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_ipam::manager::AllocatorSet;
use sartd_kubernetes::{
    context::{Ctx, State},
    controller,
    crd::address_block::AddressBlock,
    fixture::{
        reconciler::{test_address_block_lb, test_address_block_lb_non_default},
        test_trace,
    },
};

use crate::common::{cleanup_kind, setup_kind};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_address_block() {
    dbg!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

    dbg!("Getting kube client");
    let client = Client::try_default().await.unwrap();

    let allocator_set = Arc::new(AllocatorSet::new());
    let ctx = State::default().to_context_with::<Arc<AllocatorSet>>(
        client.clone(),
        30,
        allocator_set.clone(),
    );

    dbg!("Creating an AddressBlock resource");
    let ab = test_address_block_lb();
    let ab_api = Api::<AddressBlock>::all(ctx.client().clone());
    let ssapply = PatchParams::apply("ctrltest");
    let ab_patch = Patch::Apply(ab.clone());
    ab_api
        .patch(&ab.name_any(), &ssapply, &ab_patch)
        .await
        .unwrap();

    let applied_ab = ab_api.get(&ab.name_any()).await.unwrap();

    dbg!("Reconciling AddressBlock");
    controller::reconciler::address_block::reconciler(Arc::new(applied_ab.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking the block is registered in allocator set");
    {
        let alloc_set = allocator_set.clone();
        let alloc_set_inner = alloc_set.inner.lock().unwrap();
        let _block = alloc_set_inner.get(&applied_ab.name_any()).unwrap();
        assert_eq!(Some(applied_ab.name_any()), alloc_set_inner.auto_assign);
    }

    dbg!("Creating another AddressBlock");
    let ab_another = test_address_block_lb_non_default();
    let ab_patch_another = Patch::Apply(ab_another.clone());
    ab_api
        .patch(&ab_another.name_any(), &ssapply, &ab_patch_another)
        .await
        .unwrap();

    let applied_ab_another = ab_api.get(&ab_another.name_any()).await.unwrap();

    dbg!("Reconciling AddressBlock");
    controller::reconciler::address_block::reconciler(
        Arc::new(applied_ab_another.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    dbg!("Chencking the block is registered in allocator set");
    {
        let alloc_set = allocator_set.clone();
        let alloc_set_inner = alloc_set.inner.lock().unwrap();
        let _block = alloc_set_inner.get(&applied_ab_another.name_any()).unwrap();
        assert_eq!(Some(applied_ab.name_any()), alloc_set_inner.auto_assign);
    }

    dbg!("Patching to change auto assign");
    let mut ab_another_auto_assign = ab_another.clone();
    ab_another_auto_assign.spec.auto_assign = true;
    let ab_patch_another_auto_assign = Patch::Apply(ab_another_auto_assign.clone());
    ab_api
        .patch(
            &ab_another_auto_assign.name_any(),
            &ssapply,
            &ab_patch_another_auto_assign,
        )
        .await
        .unwrap();

    let applied_ab_another_auto_assign = ab_api
        .get(&ab_another_auto_assign.name_any())
        .await
        .unwrap();

    dbg!("Failing to reconcile AddressBlock");
    let _err = controller::reconciler::address_block::reconciler(
        Arc::new(applied_ab_another_auto_assign.clone()),
        ctx.clone(),
    )
    .await
    .unwrap_err();

    dbg!("Making diable auto assign");
    let mut ab_disable_auto_assign = ab.clone();
    ab_disable_auto_assign.spec.auto_assign = false;
    let ab_patch_disable_auto_assign = Patch::Apply(ab_disable_auto_assign.clone());
    ab_api
        .patch(
            &ab_disable_auto_assign.name_any(),
            &ssapply,
            &ab_patch_disable_auto_assign,
        )
        .await
        .unwrap();

    let applied_ab_disable_auto_assign = ab_api
        .get(&ab_disable_auto_assign.name_any())
        .await
        .unwrap();

    dbg!("Reconciling AddressBlock");
    controller::reconciler::address_block::reconciler(
        Arc::new(applied_ab_disable_auto_assign),
        ctx.clone(),
    )
    .await
    .unwrap();

    dbg!("Chencking the block's auto assign is disabled");
    {
        let alloc_set = allocator_set.clone();
        let alloc_set_inner = alloc_set.inner.lock().unwrap();
        assert_eq!(None, alloc_set_inner.auto_assign);
    }

    dbg!("Changing the block to auto assignable");
    let applied_ab_another_auto_assign = ab_api
        .get(&ab_another_auto_assign.name_any())
        .await
        .unwrap();

    dbg!("Reconciling AddressBlock");
    controller::reconciler::address_block::reconciler(
        Arc::new(applied_ab_another_auto_assign.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    dbg!("Chencking auto assign is set");
    {
        let alloc_set = allocator_set.clone();
        let alloc_set_inner = alloc_set.inner.lock().unwrap();
        assert_eq!(
            Some(applied_ab_another_auto_assign.name_any()),
            alloc_set_inner.auto_assign
        );
    }

    let dummy_addr = IpAddr::from_str("10.0.0.1").unwrap();
    dbg!("Inserting dummy allocation");
    {
        let alloc_set = allocator_set.clone();
        let mut alloc_set_inner = alloc_set.inner.lock().unwrap();
        let block = alloc_set_inner.get_mut(&ab.name_any()).unwrap();
        block.allocator.allocate(&dummy_addr, false).unwrap();
    }

    dbg!("Deleting AddressBlock");
    ab_api
        .delete(&ab.name_any(), &DeleteParams::default())
        .await
        .unwrap();

    let ab_deleted = ab_api.get(&ab.name_any()).await.unwrap();

    dbg!("Checking the deletion timestamp");
    assert!(ab_deleted.metadata.deletion_timestamp.is_some());

    dbg!("Failing to clean up AddressBlock");
    let _err = controller::reconciler::address_block::reconciler(
        Arc::new(ab_deleted.clone()),
        ctx.clone(),
    )
    .await
    .unwrap_err();

    dbg!("Removing dummy allocation");
    {
        let alloc_set = allocator_set.clone();
        let mut alloc_set_inner = alloc_set.inner.lock().unwrap();
        let block = alloc_set_inner.get_mut(&ab.name_any()).unwrap();
        block.allocator.release(&dummy_addr).unwrap();
    }

    dbg!("Cleaning up AddressBlock");
    controller::reconciler::address_block::reconciler(Arc::new(ab_deleted.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Chencking block is deleted");
    {
        let alloc_set = allocator_set.clone();
        let alloc_set_inner = alloc_set.inner.lock().unwrap();
        let res = alloc_set_inner.get(&ab_deleted.name_any());
        assert!(res.is_none());
    }

    dbg!("Cleaning up a kind cluster");
    cleanup_kind();
}
