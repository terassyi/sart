use std::sync::Arc;

use kube::{
    api::{DeleteParams, Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_kubernetes::{
    agent,
    context::State,
    crd::{
        bgp_peer::BGPPeer,
        node_bgp::{NodeBGP, NodeBGPCondition, NodeBGPConditionReason, NodeBGPConditionStatus},
    },
    fixture::test_trace,
};

use crate::common::{cleanup_kind, setup_kind, test_node_bgp};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_agent_node_bgp() {
    tracing::info!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

    tracing::info!("Starting the mock bgp server api server");
    tokio::spawn(async move {
        sartd_mock::bgp::run(5000).await;
    });

    tracing::info!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let ctx = State::default().to_context(client.clone(), 30);

    let nb = test_node_bgp();
    let nb_api = Api::<NodeBGP>::all(ctx.client.clone());
    let ssapply = PatchParams::apply("ctrltest");

    let nb_patch = Patch::Apply(nb.clone());

    tracing::info!("Creating the NodeBGP resource");
    nb_api
        .patch(&nb.name_any(), &ssapply, &nb_patch)
        .await
        .unwrap();

    let applied_nb = nb_api.get(&nb.name_any()).await.unwrap();

    tracing::info!("Reconciling the resource");
    agent::reconciler::node_bgp::reconciler(Arc::new(applied_nb.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Checking NodeBGP's status");
    let applied_nb = nb_api.get(&nb.name_any()).await.unwrap();
    let binding = applied_nb
        .status
        .as_ref()
        .unwrap()
        .conditions
        .clone()
        .unwrap();
    let last_cond = binding.last().unwrap();
    assert_eq!(
        &NodeBGPCondition {
            status: NodeBGPConditionStatus::Available,
            reason: NodeBGPConditionReason::Configured,
        },
        last_cond
    );

    tracing::info!("Reconciling the resource again because of requeue");
    agent::reconciler::node_bgp::reconciler(Arc::new(applied_nb.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Checking NodeBGP's status");
    let applied_nb = nb_api.get(&nb.name_any()).await.unwrap();
    let binding = applied_nb
        .status
        .as_ref()
        .unwrap()
        .conditions
        .clone()
        .unwrap();
    let last_cond = binding.last().unwrap();
    assert_eq!(
        &NodeBGPCondition {
            status: NodeBGPConditionStatus::Available,
            reason: NodeBGPConditionReason::Configured,
        },
        last_cond
    );

    tracing::info!("Reconciling the resource again because of requeue");
    let applied_nb = nb_api.get(&nb.name_any()).await.unwrap();
    agent::reconciler::node_bgp::reconciler(Arc::new(applied_nb.clone()), ctx.clone())
        .await
        .unwrap();

    let bp_api = Api::<BGPPeer>::all(ctx.client.clone());
    let _created_bp = bp_api
        .get(&nb.spec.peers.as_ref().unwrap()[0].name)
        .await
        .unwrap();

    tracing::info!("Cleaning up NodeBGP");
    nb_api
        .delete(&nb.name_any(), &DeleteParams::default())
        .await
        .unwrap();

    let deleted_nb = nb_api.get(&nb.name_any()).await.unwrap();
    agent::reconciler::node_bgp::reconciler(Arc::new(deleted_nb.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Cleaning up a kind cluster");
    cleanup_kind();
}
