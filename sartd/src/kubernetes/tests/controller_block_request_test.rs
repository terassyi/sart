
use std::sync::Arc;

use kube::{api::{Patch, PatchParams}, Api, ResourceExt};
use sartd_kubernetes::{controller, crd::{address_pool::AddressPool, block_request::BlockRequest}, fixture::reconciler::{test_address_pool_pod, test_block_request}};

use crate::common::{cleanup_kind, setup_kind};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn intergration_test_block_request() {
	dbg!("Creating a kind cluster");
	setup_kind();

    test_trace().await;

    dbg!("Getting kube client");
    let client = Client::try_default().await.unwrap();

    let ctx = State::default();

	dbg!("Creating AddressPool");
	let ap = test_address_pool_pod();
	let ap_api = Api::<AddressPool>::all(ctx.client.clone());
    let ssapply = PatchParams::apply("ctrltest");
	let ap_patch = Patch::Apply(ap.clone());
	ap_api.patch(&ap.name_any(), &ssapply, &ap_patch).await.unwrap();

	let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();

	dbg!("Creating BlockRequest");
	let br = test_block_request();
	let br_api = Api::<BlockRequest>::all(ctx.client.clone());
	let br_patch = Patch::Apply(br.clone());
	br_api.patch(&br.name_any(), &ssapply, &br_patch).await.unwrap();

	let applied_br = br_api.get(&br.name_any()).await.unwrap();

	dbg!("Reconciling BlockRequest");
	controller::reconciler::block_request::reconciler(Arc::new(applied_br.clone()), ctx.clone()).await.unwrap();

	dbg!("Checking AddressPool's status");
	let applied_ap = ap_api.get(&ap.name_any()).await.unwrap();
	let requested = applied_ap.status.unwrap().requested.unwrap();
	assert_eq!(len(requested), 1);
	assert_eq!(requested[0], br.name_any());

    dbg!("Cleaning up a kind cluster");
    cleanup_kind();
}
