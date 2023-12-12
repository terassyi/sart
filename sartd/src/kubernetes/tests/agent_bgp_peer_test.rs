use std::{
    net::Ipv4Addr,
    str::FromStr,
    sync::{Arc, Mutex},
};

use kube::{
    api::{DeleteParams, Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_kubernetes::{
    agent,
    context::State,
    crd::{
        bgp_peer::{BGPPeer, BGPPeerConditionStatus},
        node_bgp::NodeBGP,
    },
    fixture::test_trace,
};
use sartd_mock::bgp::MockBgpApiServerInner;

use crate::common::{cleanup_kind, setup_kind, test_bgp_peer, test_node_bgp};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_agent_bgp_peer() {
    dbg!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

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

    dbg!("Preraring NodeBGP resource");
    let nb = test_node_bgp();
    let nb_api = Api::<NodeBGP>::all(ctx.client.clone());
    let ssapply = PatchParams::apply("ctrltest");
    let nb_patch = Patch::Apply(nb.clone());

    nb_api
        .patch(&nb.name_any(), &ssapply, &nb_patch)
        .await
        .unwrap();

    let bp = test_bgp_peer();
    let bp_api = Api::<BGPPeer>::all(ctx.client.clone());

    let bp_patch = Patch::Apply(bp.clone());

    dbg!("Creating the BGPPeer resource");
    bp_api
        .patch(&bp.name_any(), &ssapply, &bp_patch)
        .await
        .unwrap();

    let applied_bp = bp_api.get(&bp.name_any()).await.unwrap();

    dbg!("Reconciling the resource");
    agent::reconciler::bgp_peer::reconciler(Arc::new(applied_bp.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking a peer is stored into mock server");
    {
        let mut mock = inner.lock().unwrap();
        let mut peer = mock
            .peers
            .get_mut(&Ipv4Addr::from_str("172.0.0.1").unwrap());
        assert!(peer.is_some());

        dbg!("Changing the peer state");
        peer.as_mut()
            .unwrap()
            .set_state(sartd_proto::sart::peer::State::Established);
    }

    dbg!("Reconciling the resource again");
    agent::reconciler::bgp_peer::reconciler(Arc::new(applied_bp.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking the peer status");
    let applied_bp = bp_api.get(&bp.name_any()).await.unwrap();
    assert_eq!(
        BGPPeerConditionStatus::Established,
        applied_bp
            .status
            .as_ref()
            .unwrap()
            .conditions
            .as_ref()
            .unwrap()
            .last()
            .unwrap()
            .status
    );

    dbg!("Cleaning up BGPPeer");
    bp_api
        .delete(&bp.name_any(), &DeleteParams::default())
        .await
        .unwrap();

    let deleted_bp = bp_api.get(&bp.name_any()).await.unwrap();

    dbg!("Reconciling BGPPeer");
    agent::reconciler::bgp_peer::reconciler(Arc::new(deleted_bp.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking the peer is deleted");
    {
        let mock = inner.lock().unwrap();
        let peer = mock.peers.get(&Ipv4Addr::from_str("172.0.0.1").unwrap());
        assert!(peer.is_none());
    }

    dbg!("Cleaning up a kind cluster");
    cleanup_kind();
}
