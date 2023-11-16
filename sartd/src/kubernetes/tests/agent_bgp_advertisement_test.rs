use std::sync::{Arc, Mutex};

use kube::{
    api::{Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_kubernetes::{
    agent::{self, reconciler::node_bgp::ENV_HOSTNAME},
    context::State,
    crd::{
        bgp_advertisement::{AdvertiseStatus, BGPAdvertisement},
        bgp_peer::{BGPPeer, BGPPeerCondition, BGPPeerConditionStatus, BGPPeerStatus},
        node_bgp::NodeBGP,
    },
    fixture::test_trace,
    util::get_namespace,
};
use sartd_mock::bgp::MockBgpApiServerInner;

use crate::common::{
    cleanup_kind, setup_kind, test_bgp_advertisement, test_bgp_peer, test_node_bgp, KIND_NODE_CP,
};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_agent_bgp_advertisement() {
    dbg!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

    dbg!("Setting env value");
    std::env::set_var(ENV_HOSTNAME, KIND_NODE_CP);

    dbg!("Starting the mock bgp server api server");
    let inner = Arc::new(Mutex::new(MockBgpApiServerInner::new_with(
        65000,
        "172.0.0.1",
    )));
    let cloned_inner = inner.clone();
    tokio::spawn(async move {
        sartd_mock::bgp::run_with(cloned_inner, 5000).await;
    });

    dbg!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let ctx = State::default().to_context(client.clone(), 30);

    dbg!("Creating NodeBGP");
    let nb = test_node_bgp();
    let nb_api = Api::<NodeBGP>::all(ctx.client.clone());
    let nb_patch = Patch::Apply(nb.clone());

    let ssapply = PatchParams::apply("ctrltest");

    nb_api
        .patch(&nb.name_any(), &ssapply, &nb_patch)
        .await
        .unwrap();

    dbg!("Creating BGPPeer");
    let mut bp = test_bgp_peer();
    bp.status = Some(BGPPeerStatus {
        conditions: Some(vec![BGPPeerCondition {
            status: BGPPeerConditionStatus::Established,
            reason: String::new(),
        }]),
    });
    let bp_patch = Patch::Apply(bp.clone());
    let bp_api = Api::<BGPPeer>::all(ctx.client.clone());
    bp_api
        .patch(&bp.name_any(), &ssapply, &bp_patch)
        .await
        .unwrap();

    bp_api
        .patch_status(&bp.name_any(), &ssapply, &bp_patch)
        .await
        .unwrap();

    let ba = test_bgp_advertisement();
    let ns = get_namespace(&ba).unwrap();
    let ba_api = Api::<BGPAdvertisement>::namespaced(ctx.client.clone(), &ns);
    let ba_patch = Patch::Apply(ba.clone());

    ba_api
        .patch(&ba.name_any(), &ssapply, &ba_patch)
        .await
        .unwrap();

    ba_api
        .patch_status(&ba.name_any(), &ssapply, &ba_patch)
        .await
        .unwrap();

    dbg!("Reconciling BGPAdvertisement");
    let applied_ba = ba_api.get(&ba.name_any()).await.unwrap();
    agent::reconciler::bgp_advertisement::reconciler(Arc::new(applied_ba.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking the status");
    let applied_ba = ba_api.get(&ba.name_any()).await.unwrap();
    assert_eq!(
        &AdvertiseStatus::Advertised,
        applied_ba
            .status
            .as_ref()
            .unwrap()
            .peers
            .as_ref()
            .unwrap()
            .get("test-peer1")
            .unwrap()
    );

    dbg!("Checking the path exists");
    {
        let mock = inner.lock().unwrap();
        assert_eq!(1, mock.paths.len());
    }

    let mut deleted_ba = ba.clone();
    let status = deleted_ba
        .status
        .as_mut()
        .unwrap()
        .peers
        .as_mut()
        .unwrap()
        .get_mut("test-peer1")
        .unwrap();
    *status = AdvertiseStatus::Withdraw;

    let deleted_ba_patch = Patch::Merge(deleted_ba.clone());
    ba_api
        .patch_status(&deleted_ba.name_any(), &ssapply, &deleted_ba_patch)
        .await
        .unwrap();

    dbg!("Reconciling BGPAdvertisement");
    let deleted_ba = ba_api.get(&ba.name_any()).await.unwrap();
    agent::reconciler::bgp_advertisement::reconciler(Arc::new(deleted_ba.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking the path doesn't exist");
    {
        let mock = inner.lock().unwrap();
        assert_eq!(0, mock.paths.len());
    }

    dbg!("Checking the status");
    let applied_ba = ba_api.get(&ba.name_any()).await.unwrap();
    let status = applied_ba.status.as_ref().unwrap().peers.as_ref().unwrap();
    assert!(status.get("test-peer1").is_none());

    dbg!("Cleaning up a kind cluster");
    cleanup_kind();
}
