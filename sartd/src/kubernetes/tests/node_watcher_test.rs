use std::{collections::BTreeMap, sync::Arc};

use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_kubernetes::{
    context::State,
    controller::{self, reconciler::node_watcher::NODE_FINALIZER},
    crd::{
        bgp_peer_template::BGPPeerTemplate,
        cluster_bgp::{ClusterBGP, ClusterBGPStatus},
        node_bgp::NodeBGP,
    },
    fixture::{
        reconciler::{test_bgp_peer_tmpl, test_cluster_bgp},
        test_trace,
    },
};

use crate::common::{cleanup_kind, kubectl_label, setup_kind, KIND_NODE_CP};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_controller_node_watcher() {
    dbg!("Creating a kind cluster");
    setup_kind();

    test_trace().await;

    kubectl_label("nodes", KIND_NODE_CP, "bgp=c");

    dbg!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let ctx = State::default().to_context(client.clone(), 30);

    let mut cb = test_cluster_bgp();
    cb.spec.node_selector = Some(BTreeMap::from([("bgp".to_string(), "a".to_string())]));
    let cb_api = Api::<ClusterBGP>::all(ctx.client.clone());
    let ssapply = PatchParams::apply("ctrltest");
    let cb_patch = Patch::Apply(cb.clone());

    dbg!("Creating the ClusterBGP resource");
    cb_api
        .patch(&cb.name_any(), &ssapply, &cb_patch)
        .await
        .unwrap();

    let tmpl = test_bgp_peer_tmpl();
    let tmpl_api = Api::<BGPPeerTemplate>::all(ctx.client.clone());
    let tmpl_patch = Patch::Apply(tmpl.clone());
    tmpl_api
        .patch(&tmpl.name_any(), &ssapply, &tmpl_patch)
        .await
        .unwrap();

    let applied_cb = cb_api.get(&cb.name_any()).await.unwrap();

    // do reconcile
    dbg!("Reconciling the resource when creating");
    controller::reconciler::cluster_bgp::reconciler(Arc::new(applied_cb.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking ClusterBGP.status is empty");
    let applied_cb = cb_api.get(&cb.name_any()).await.unwrap();
    let nodes = applied_cb
        .status
        .clone()
        .unwrap_or(ClusterBGPStatus::default())
        .nodes
        .unwrap_or(Vec::new());
    assert!(nodes.is_empty());

    dbg!("Patching bgp=a label to Node");
    kubectl_label("nodes", KIND_NODE_CP, "bgp=a");

    dbg!("Reconciling Node");
    let node_api = Api::<Node>::all(ctx.client.clone());
    let mut node = node_api.get(KIND_NODE_CP).await.unwrap();
    node.finalizers_mut().push(NODE_FINALIZER.to_string());

    controller::reconciler::node_watcher::reconciler(Arc::new(node), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking ClusterBGP's status is updated");
    let applied_cb = cb_api.get(&applied_cb.name_any()).await.unwrap();
    let desired_nodes = applied_cb
        .status
        .clone()
        .unwrap()
        .desired_nodes
        .unwrap_or(Vec::new());
    assert_eq!(desired_nodes, vec![KIND_NODE_CP.to_string()]);

    // do reconcile
    dbg!("Reconciling ClusterBGP");
    let applied_cb = cb_api.get(&cb.name_any()).await.unwrap();
    controller::reconciler::cluster_bgp::reconciler(Arc::new(applied_cb.clone()), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking ClusterBGP's status is updated after the reconciliation");
    let applied_cb = cb_api.get(&applied_cb.name_any()).await.unwrap();
    let desired_nodes = applied_cb
        .status
        .clone()
        .unwrap()
        .desired_nodes
        .unwrap_or(Vec::new());
    assert_eq!(desired_nodes, vec![KIND_NODE_CP.to_string()]);

    let actual_nodes = applied_cb
        .status
        .clone()
        .unwrap()
        .nodes
        .unwrap_or(Vec::new());
    assert_eq!(actual_nodes, vec![KIND_NODE_CP.to_string()]);

    dbg!("Checking NodeBGP is created");
    let node_bgp_api = Api::<NodeBGP>::all(ctx.client.clone());
    let nb_opt = node_bgp_api.get_opt(KIND_NODE_CP).await.unwrap();
    assert!(nb_opt.is_some());

    dbg!("Patching bgp=b label to Node");
    kubectl_label("nodes", KIND_NODE_CP, "bgp=b");

    dbg!("Cheking node's label");
    let mut node = node_api.get(KIND_NODE_CP).await.unwrap();
    node.finalizers_mut().push(NODE_FINALIZER.to_string());

    let b = node.labels().get("bgp");
    assert_eq!(b, Some(&"b".to_string()));

    dbg!("Reconciling Node");
    controller::reconciler::node_watcher::reconciler(Arc::new(node), ctx.clone())
        .await
        .unwrap();

    dbg!("Checking ClusterBGP's status is updated");
    let applied_cb = cb_api.get(&applied_cb.name_any()).await.unwrap();
    let updated_nodes = applied_cb
        .status
        .unwrap()
        .desired_nodes
        .unwrap_or(Vec::new());
    assert!(updated_nodes.is_empty());

    dbg!("Checking NodeBGP's label is synchronized");
    let nb = node_bgp_api.get(KIND_NODE_CP).await.unwrap();
    let value = nb.labels().get("bgp").unwrap();
    assert_eq!(value, "b");

    dbg!("Clean up kind cluster");
    cleanup_kind();
}
