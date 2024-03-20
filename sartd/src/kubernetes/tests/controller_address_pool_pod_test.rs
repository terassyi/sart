use std::sync::Arc;

use common::{cleanup_kind, setup_kind};

use kube::{
    api::{ListParams, Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_kubernetes::{
    context::State,
    controller,
    crd::{
        address_block::AddressBlock,
        address_pool::{AddressPool, AddressPoolStatus, ADDRESS_POOL_ANNOTATION},
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
    dbg!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

    dbg!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let ctx = State::default().to_context(client.clone(), 30);

    let ap = test_address_pool_pod();

    dbg!("Creating an AddressPool");
    let ap_api = Api::<AddressPool>::all(ctx.client.clone());
    let ssapply = PatchParams::apply("ctrltest");
    let ap_patch = Patch::Apply(ap.clone());
    ap_api
        .patch(&ap.name_any(), &ssapply, &ap_patch)
        .await
        .unwrap();

    let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();

    dbg!("Reconciling AddressPool");
    controller::reconciler::address_pool::reconciler(Arc::new(applied_ap.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Getting the applied pool");
    let mut applied_ap = ap_api.get(&ap.name_any()).await.unwrap();

    assert!(applied_ap.status.is_none());

    dbg!("Creating BlockRequest");
    let br = test_block_request();
    let br_api = Api::<BlockRequest>::all(ctx.client.clone());
    let br_patch = Patch::Apply(br.clone());
    br_api
        .patch(&br.name_any(), &ssapply, &br_patch)
        .await
        .unwrap();

    applied_ap.status = Some(AddressPoolStatus {
        requested: Some(vec![br.name_any()]),
        allocated: None,
        released: None,
    });

    dbg!("Updating AddressPool status");
    applied_ap.metadata.managed_fields = None;
    let ap_patch = Patch::Apply(applied_ap);
    ap_api
        .patch_status(&ap.name_any(), &ssapply, &ap_patch)
        .await
        .unwrap();

    let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();

    dbg!("Reconciling AddressPool");
    controller::reconciler::address_pool::reconciler(Arc::new(applied_ap.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking status is updated");
    let mut applied_ap = ap_api.get(&ap.name_any()).await.unwrap();
    let allocated = applied_ap
        .status
        .as_ref()
        .unwrap()
        .allocated
        .as_ref()
        .unwrap();
    assert_eq!(1, allocated.len());
    let block_sart = allocated
        .get(&format!("{}-{}-10.0.0.0", br.spec.pool, br.spec.node))
        .unwrap();
    assert_eq!(0, *block_sart);

    let requested = applied_ap
        .status
        .as_mut()
        .unwrap()
        .requested
        .as_mut()
        .unwrap();
    assert!(requested.is_empty());

    dbg!("Checking the block is created");
    let ab_api = Api::<AddressBlock>::all(ctx.client.clone());
    let ab_opt = ab_api
        .get_opt(&format!("{}-{}-10.0.0.0", br.spec.pool, br.spec.node))
        .await
        .unwrap();
    assert!(ab_opt.is_some());

    dbg!("Requesting new block");
    requested.push(br.name_any());
    applied_ap.metadata.managed_fields = None;
    let ap_patch = Patch::Apply(applied_ap.clone());
    let mut ssapply_force = ssapply.clone();
    ssapply_force.force = true;
    ap_api
        .patch_status(&ap.name_any(), &ssapply_force, &ap_patch)
        .await
        .unwrap();

    let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();

    dbg!("Reconciling AddressPool");
    controller::reconciler::address_pool::reconciler(Arc::new(applied_ap.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking status is updated");
    let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();
    let allocated = applied_ap
        .status
        .as_ref()
        .unwrap()
        .allocated
        .as_ref()
        .unwrap();
    assert_eq!(2, allocated.len());
    let block_sart = allocated
        .get(&format!("{}-{}-10.0.0.32", br.spec.pool, br.spec.node))
        .unwrap();
    assert_eq!(32, *block_sart);

    let requested = applied_ap
        .status
        .as_ref()
        .unwrap()
        .requested
        .as_ref()
        .unwrap();
    assert!(requested.is_empty());

    dbg!("Checking the block is created");
    let ab_opt = ab_api
        .get_opt(&format!("{}-{}-10.0.0.32", br.spec.pool, br.spec.node))
        .await
        .unwrap();
    assert!(ab_opt.is_some());

    dbg!("Releasing the block");
    let mut applied_ap = ap_api.get(&ap.name_any()).await.unwrap();
    applied_ap.metadata.managed_fields = None;
    applied_ap.status.as_mut().unwrap().released =
        Some(vec![format!("{}-{}-10.0.0.0", br.spec.pool, br.spec.node)]);
    let ap_patch = Patch::Apply(applied_ap.clone());
    let mut ssapply_force = ssapply.clone();
    ssapply_force.force = true;
    ap_api
        .patch_status(&ap.name_any(), &ssapply_force, &ap_patch)
        .await
        .unwrap();

    let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();

    dbg!("Reconciling AddressPool");
    controller::reconciler::address_pool::reconciler(Arc::new(applied_ap.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Remaining the status.released field");
    let released = applied_ap
        .status
        .as_ref()
        .unwrap()
        .released
        .as_ref()
        .unwrap();
    assert_eq!(1, released.len());

    dbg!("Checking the block is deleted");
    let ab_opt = ab_api
        .get_opt(&format!("{}-{}-10.0.0.0", br.spec.pool, br.spec.node))
        .await
        .unwrap();
    assert!(ab_opt.is_none());

    dbg!("Reconciling AddressPool");
    let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();
    controller::reconciler::address_pool::reconciler(Arc::new(applied_ap.clone()), ctx.clone())
        .await
        .unwrap();

    let mut applied_ap = ap_api.get(&ap.name_any()).await.unwrap();

    dbg!("Removing the status.released field");
    let released = applied_ap
        .status
        .as_ref()
        .unwrap()
        .released
        .as_ref()
        .unwrap();
    assert!(released.is_empty());

    dbg!("Changing auto assign to false");
    applied_ap.spec.auto_assign = Some(false);

    dbg!("Reconciling AddressPool");
    controller::reconciler::address_pool::reconciler(Arc::new(applied_ap.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking blocks are changed to auto_assign=false");
    let list_params = ListParams::default().labels(&format!(
        "{}={}",
        ADDRESS_POOL_ANNOTATION,
        applied_ap.name_any()
    ));
    let ab_list = ab_api.list(&list_params).await.unwrap();
    for ab in ab_list.iter() {
        assert!(!ab.spec.auto_assign);
    }

    dbg!("Cleaning up a kind cluster");
    cleanup_kind();
}
