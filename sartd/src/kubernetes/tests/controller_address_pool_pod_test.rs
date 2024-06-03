use std::sync::{Arc, Mutex};

use common::{cleanup_kind, setup_kind};

use kube::{
    api::{ListParams, Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_ipam::manager::BlockAllocator;
use sartd_kubernetes::{
    controller::{
        self,
        context::{Ctx, State},
        metrics::Metrics,
    },
    crd::{
        address_block::AddressBlock,
        address_pool::{AddressPool, ADDRESS_POOL_ANNOTATION},
        block_request::BlockRequest,
    },
    fixture::{
        reconciler::{test_address_pool_pod, test_block_request},
        test_trace,
    },
};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn test_address_pool_pod_handling_request() {
    tracing::info!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

    tracing::info!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let block_allocator = Arc::new(BlockAllocator::default());
    let ctx = State::default().to_context_with(
        client.clone(),
        30,
        block_allocator,
        Arc::new(Mutex::new(Metrics::default())),
    );

    let ap = test_address_pool_pod();

    tracing::info!("Creating an AddressPool");
    let ap_api = Api::<AddressPool>::all(ctx.client().clone());
    let ssapply = PatchParams::apply("ctrltest");
    let ap_patch = Patch::Apply(ap.clone());
    ap_api
        .patch(&ap.name_any(), &ssapply, &ap_patch)
        .await
        .unwrap();

    let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();

    tracing::info!("Reconciling AddressPool");
    controller::reconciler::address_pool::reconciler(Arc::new(applied_ap.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Getting the applied pool");
    let mut applied_ap = ap_api.get(&ap.name_any()).await.unwrap();

    assert!(applied_ap.status.is_none());

    tracing::info!("Creating BlockRequest");
    let br = test_block_request();
    let br_api = Api::<BlockRequest>::all(ctx.client().clone());
    let br_patch = Patch::Apply(br.clone());
    br_api
        .patch(&br.name_any(), &ssapply, &br_patch)
        .await
        .unwrap();

    let ab_api = Api::<AddressBlock>::all(ctx.client().clone());

    tracing::info!("Changing auto assign to false");
    applied_ap.spec.auto_assign = Some(false);

    tracing::info!("Reconciling AddressPool");
    controller::reconciler::address_pool::reconciler(Arc::new(applied_ap.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Checking blocks are changed to auto_assign=false");
    let list_params = ListParams::default().labels(&format!(
        "{}={}",
        ADDRESS_POOL_ANNOTATION,
        applied_ap.name_any()
    ));
    let ab_list = ab_api.list(&list_params).await.unwrap();
    for ab in ab_list.iter() {
        assert!(!ab.spec.auto_assign);
    }

    tracing::info!("Cleaning up a kind cluster");
    cleanup_kind();
}
