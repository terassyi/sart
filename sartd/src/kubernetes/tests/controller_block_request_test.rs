use std::sync::Arc;

use kube::{
    api::{Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_ipam::manager::BlockAllocator;
use sartd_kubernetes::{
    context::{Ctx, State},
    controller,
    crd::{address_pool::AddressPool, block_request::BlockRequest},
    fixture::{
        reconciler::{test_address_pool_pod, test_block_request},
        test_trace,
    },
};

use crate::common::{cleanup_kind, setup_kind};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_block_request() {
    tracing::info!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

    tracing::info!("Getting kube client");
    let client = Client::try_default().await.unwrap();

    let block_allocator = Arc::new(BlockAllocator::default());

    let ctx = State::default().to_context_with(client.clone(), 30, block_allocator);

    tracing::info!("Creating AddressPool");
    let ap = test_address_pool_pod();
    let ap_api = Api::<AddressPool>::all(ctx.client().clone());
    let ssapply = PatchParams::apply("ctrltest");
    let ap_patch = Patch::Apply(ap.clone());
    ap_api
        .patch(&ap.name_any(), &ssapply, &ap_patch)
        .await
        .unwrap();
    let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();
    controller::reconciler::address_pool::reconciler(Arc::new(applied_ap), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Reconciling AddressPool");

    tracing::info!("Creating BlockRequest");
    let br = test_block_request();
    let br_api = Api::<BlockRequest>::all(ctx.client().clone());
    let br_patch = Patch::Apply(br.clone());
    br_api
        .patch(&br.name_any(), &ssapply, &br_patch)
        .await
        .unwrap();

    let applied_br = br_api.get(&br.name_any()).await.unwrap();

    tracing::info!("Reconciling BlockRequest");
    controller::reconciler::block_request::reconciler(Arc::new(applied_br.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Cleaning up a kind cluster");
    cleanup_kind();
}
