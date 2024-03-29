use std::sync::Arc;

use common::{cleanup_kind, setup_kind};

use kube::{
    api::{Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_kubernetes::{
    context::State,
    controller,
    crd::{address_block::AddressBlock, address_pool::AddressPool},
    fixture::{reconciler::test_address_pool_lb, test_trace},
};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_address_pool() {
    dbg!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

    dbg!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let ctx = State::default().to_context(client.clone(), 30);

    let ap = test_address_pool_lb();

    dbg!("Creating an AddressPool resource");
    let ap_api = Api::<AddressPool>::all(ctx.client.clone());
    let ssapply = PatchParams::apply("ctrltest");
    let ap_patch = Patch::Apply(ap.clone());
    ap_api
        .patch(&ap.name_any(), &ssapply, &ap_patch)
        .await
        .unwrap();

    let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();

    // do reconcile
    dbg!("Reconciling AddressPool");
    controller::reconciler::address_pool::reconciler(Arc::new(applied_ap.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Getting a AddressBlock resource created by AddressPool");
    let ab_api = Api::<AddressBlock>::all(ctx.client.clone());
    let ab = ab_api.get(&applied_ap.name_any()).await.unwrap();

    dbg!("Checking created block");
    assert_eq!(applied_ap.spec.cidr, ab.spec.cidr);
    assert_eq!(
        applied_ap.spec.auto_assign.unwrap_or_default(),
        ab.spec.auto_assign
    );

    dbg!("Cleaning up a kind cluster");
    cleanup_kind();
}
