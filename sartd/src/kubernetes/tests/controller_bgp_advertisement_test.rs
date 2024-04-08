use std::{collections::BTreeMap, sync::Arc};

use kube::{
    api::{DeleteParams, Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_kubernetes::{
    context::State,
    controller,
    crd::bgp_advertisement::{AdvertiseStatus, BGPAdvertisement, BGPAdvertisementStatus},
    fixture::{reconciler::test_bgp_advertisement_svc, test_trace},
    util::get_namespace,
};

use crate::common::{cleanup_kind, setup_kind};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_controller_bgp_advertisement() {
    tracing::info!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

    tracing::info!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let ctx = State::default().to_context(client.clone(), 30);

    let mut ba = test_bgp_advertisement_svc();
    ba.status = Some(BGPAdvertisementStatus {
        peers: Some(BTreeMap::from([
            (
                common::KIND_NODE_CP.to_string(),
                AdvertiseStatus::NotAdvertised,
            ),
            ("dummy".to_string(), AdvertiseStatus::NotAdvertised),
        ])),
    });

    let ns = get_namespace(&ba).unwrap();

    tracing::info!("Creating a BGPAdvertisement resource");
    let ba_api = Api::<BGPAdvertisement>::namespaced(ctx.client.clone(), &ns);
    let ssapply = PatchParams::apply("ctrltest");
    let ba_patch = Patch::Apply(ba.clone());
    ba_api
        .patch(&ba.name_any(), &ssapply, &ba_patch)
        .await
        .unwrap();

    let applied_ba = ba_api.get(&ba.name_any()).await.unwrap();

    tracing::info!("Reconciling BGPAdvertisement");
    controller::reconciler::bgp_advertisement::reconciler(
        Arc::new(applied_ba.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    tracing::info!("updating BGPAdvertisement status");
    let mut ba_advertised = ba.clone();
    ba_advertised.status = Some(BGPAdvertisementStatus {
        peers: Some(BTreeMap::from([
            (
                common::KIND_NODE_CP.to_string(),
                AdvertiseStatus::Advertised,
            ),
            ("dummy".to_string(), AdvertiseStatus::Advertised),
        ])),
    });
    let ba_advertised_patch = Patch::Merge(ba_advertised.clone());
    ba_api
        .patch_status(&ba_advertised.name_any(), &ssapply, &ba_advertised_patch)
        .await
        .unwrap();

    let applied_ba_advertised = ba_api.get(&ba_advertised.name_any()).await.unwrap();

    tracing::info!("Deleting BGPAdvertisement");
    ba_api
        .delete(&applied_ba_advertised.name_any(), &DeleteParams::default())
        .await
        .unwrap();

    let ba_deleted = ba_api.get(&applied_ba_advertised.name_any()).await.unwrap();

    tracing::info!("Checking delettion timestamp");
    assert!(ba_deleted.metadata.deletion_timestamp.is_some());

    tracing::info!("Failing to cleanup BGPAdvertisement");
    let _err = controller::reconciler::bgp_advertisement::reconciler(
        Arc::new(ba_deleted.clone()),
        ctx.clone(),
    )
    .await
    .unwrap_err();

    tracing::info!("Checking the status is moved to Withdraw");
    let ba_deleted_withdraw = ba_api.get(&ba_deleted.name_any()).await.unwrap();
    for (_name, status) in ba_deleted_withdraw
        .status
        .as_ref()
        .unwrap()
        .peers
        .as_ref()
        .unwrap()
        .iter()
    {
        assert_eq!(AdvertiseStatus::Withdraw, *status);
    }

    tracing::info!("Updating status to withdrawn");
    let mut ba_deleted_withdrawn = ba_deleted_withdraw.clone();
    ba_deleted_withdrawn.status = None;
    let ba_deleted_withdrawn_patch = Patch::Merge(ba_deleted_withdrawn.clone());
    ba_api
        .patch_status(
            &ba_deleted_withdrawn.name_any(),
            &ssapply,
            &ba_deleted_withdrawn_patch,
        )
        .await
        .unwrap();

    tracing::info!("Cleaning up BGPAdvertisement");
    controller::reconciler::bgp_advertisement::reconciler(
        Arc::new(ba_deleted_withdrawn),
        ctx.clone(),
    )
    .await
    .unwrap();

    tracing::info!("Cleaning up a kind cluster");
    cleanup_kind();
}
