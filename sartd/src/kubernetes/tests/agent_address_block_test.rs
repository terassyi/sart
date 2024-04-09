use std::{net::IpAddr, str::FromStr, sync::Arc};

use kube::{
    api::{DeleteParams, Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_ipam::manager::AllocatorSet;
use sartd_kubernetes::{
    agent::{self, reconciler::address_block::PodAllocator},
    context::{Ctx, State},
    crd::{address_block::AddressBlock, bgp_advertisement::BGPAdvertisement},
    fixture::{
        reconciler::{test_address_block_pod, test_address_block_pod2},
        test_trace,
    },
};
use tokio::sync::mpsc::unbounded_channel;

use crate::common::{cleanup_kind, setup_kind};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_address_block() {
    tracing::info!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

    tracing::info!("Getting kube client");
    let client = Client::try_default().await.unwrap();

    tracing::info!("Preparing components");
    let allocator_set = Arc::new(AllocatorSet::new());
    let (sender, mut receiver) = unbounded_channel::<AddressBlock>();
    let pod_allocator = Arc::new(PodAllocator {
        allocator: allocator_set,
        notifier: sender,
    });

    let ctx = State::default().to_context_with(client.clone(), 30, pod_allocator.clone());

    tracing::info!("Creating an AddressBlock resource");
    let ab = test_address_block_pod();
    let ab_api = Api::<AddressBlock>::all(ctx.client().clone());
    let ssapply = PatchParams::apply("ctrltest");
    let ab_patch = Patch::Apply(ab.clone());
    ab_api
        .patch(&ab.name_any(), &ssapply, &ab_patch)
        .await
        .unwrap();

    let applied_ab = ab_api.get(&ab.name_any()).await.unwrap();

    tracing::info!("Reconciling AddressBlock");
    agent::reconciler::address_block::reconciler(Arc::new(applied_ab.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Receiving the notification");
    let received_ab = receiver.recv().await.unwrap();
    assert_eq!(received_ab.name_any(), ab.name_any());

    tracing::info!("Checking the block is registered in pod_allocator");
    {
        let pod_allocator = pod_allocator.clone();
        let alloc_set_inner = pod_allocator.allocator.inner.lock().unwrap();
        let _block = alloc_set_inner.get(&applied_ab.name_any()).unwrap();
        assert_eq!(
            Some(applied_ab.spec.pool_ref.clone()),
            alloc_set_inner.auto_assign
        );
    }

    tracing::info!("Checking a BGPAdvertisement is created");
    let ba_api = Api::<BGPAdvertisement>::namespaced(client.clone(), "kube-system");
    let ba_opt = ba_api.get_opt(&applied_ab.name_any()).await.unwrap();
    assert!(ba_opt.is_some());

    tracing::info!("Creating another AddressBlock");
    let ab_another = test_address_block_pod2();
    let ab_patch_another = Patch::Apply(ab_another.clone());
    ab_api
        .patch(&ab_another.name_any(), &ssapply, &ab_patch_another)
        .await
        .unwrap();

    let applied_ab_another = ab_api.get(&ab_another.name_any()).await.unwrap();

    tracing::info!("Reconciling AddressBlock");
    agent::reconciler::address_block::reconciler(Arc::new(applied_ab_another.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Receiving the notification");
    let received_ab = receiver.recv().await.unwrap();
    assert_eq!(received_ab.name_any(), ab_another.name_any());

    tracing::info!("Checking the block is registered in pod_allocator");
    {
        let pod_allocator = pod_allocator.clone();
        let alloc_set_inner = pod_allocator.allocator.inner.lock().unwrap();
        let _block = alloc_set_inner.get(&ab_another.name_any()).unwrap();
        assert_eq!(
            Some(applied_ab.spec.pool_ref.clone()),
            alloc_set_inner.auto_assign
        );
    }

    tracing::info!("Checking a BGPAdvertisement is created");
    let ba_api = Api::<BGPAdvertisement>::namespaced(client.clone(), "kube-system");
    let ba_opt = ba_api
        .get_opt(&applied_ab_another.name_any())
        .await
        .unwrap();
    assert!(ba_opt.is_some());

    let dummy_addr = IpAddr::from_str("10.0.0.1").unwrap();
    tracing::info!("Inserting dummy allocation");
    {
        let pod_allocator = pod_allocator.clone();
        let mut alloc_set_inner = pod_allocator.allocator.inner.lock().unwrap();
        let block = alloc_set_inner.get_mut(&ab.name_any()).unwrap();
        block.allocator.allocate(&dummy_addr, false).unwrap();
    }

    tracing::info!("Deleting AddressBlock");
    ab_api
        .delete(&ab.name_any(), &DeleteParams::default())
        .await
        .unwrap();

    let ab_deleted = ab_api.get(&ab.name_any()).await.unwrap();

    tracing::info!("Checking the deletion timestamp");
    assert!(ab_deleted.metadata.deletion_timestamp.is_some());

    tracing::info!("Failing to clean up AddressBlock");
    let _err =
        agent::reconciler::address_block::reconciler(Arc::new(ab_deleted.clone()), ctx.clone())
            .await
            .unwrap_err();

    tracing::info!("Removing dummy allocation");
    {
        let pod_allocator = pod_allocator.clone();
        let mut alloc_set_inner = pod_allocator.allocator.inner.lock().unwrap();
        let block = alloc_set_inner.get_mut(&ab.name_any()).unwrap();
        block.allocator.release(&dummy_addr).unwrap();
    }

    tracing::info!("Cleaning up AddressBlock");
    agent::reconciler::address_block::reconciler(Arc::new(ab_deleted.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Checking block is deleted");
    {
        let pod_allocator = pod_allocator.clone();
        let alloc_set_inner = pod_allocator.allocator.inner.lock().unwrap();
        let res = alloc_set_inner.get(&ab_deleted.name_any());
        assert!(res.is_none());
    }

    tracing::info!("Checking a BGPAdvertisement is deleted");
    let ba_api = Api::<BGPAdvertisement>::namespaced(client.clone(), "kube-system");
    let ba_opt = ba_api.get_opt(&ab_deleted.name_any()).await.unwrap();
    assert!(ba_opt.is_none());

    tracing::info!("Cleaning up a kind cluster");
    cleanup_kind();
}
