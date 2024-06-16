use std::sync::{Arc, Mutex};

use common::{cleanup_kind, setup_kind};
use k8s_openapi::api::core::v1::Node;
use kube::api::ListParams;
use kube::ResourceExt;
use kube::{
    api::{Patch, PatchParams},
    Api, Client,
};
use sartd_kubernetes::controller;
use sartd_kubernetes::controller::context::State;
use sartd_kubernetes::controller::metrics::Metrics;
use sartd_kubernetes::crd::cluster_bgp::{AsnSelectionType, AsnSelector, ASN_LABEL};
use sartd_kubernetes::crd::node_bgp::NodeBGP;
use sartd_kubernetes::fixture::test_trace;
use sartd_kubernetes::{
    crd::{bgp_peer_template::BGPPeerTemplate, cluster_bgp::ClusterBGP},
    fixture::reconciler::{test_bgp_peer_tmpl, test_cluster_bgp},
};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_cluster_bgp_asn() {
    tracing::info!("Setting up a kind cluster");
    setup_kind();

    test_trace().await;

    tracing::info!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let ctx = State::default().to_context(client.clone(), 30, Arc::new(Mutex::new(Metrics::default())));

    let cb = test_cluster_bgp();
    let cb_api = Api::<ClusterBGP>::all(ctx.client.clone());
    let ssapply = PatchParams::apply("ctrltest");
    let cb_patch = Patch::Apply(cb.clone());

    tracing::info!("Creating the ClusterBGP resource");
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

    // To use the resource that has uid, get it from the api server again after creating.
    // To set the owner reference, we need uid.
    let applied_cb = cb_api.get(&cb.name_any()).await.unwrap();

    // do reconcile
    tracing::info!("Reconciling the resource when creating");
    controller::reconciler::cluster_bgp::reconciler(Arc::new(applied_cb.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Getting NodeBGP resources created by reconciling ClusterBGP");
    let node_api = Api::<Node>::all(ctx.client.clone());
    let nb_api = Api::<NodeBGP>::all(ctx.client.clone());
    let node_list = node_api.list(&ListParams::default()).await.unwrap();
    for node in node_list.iter() {
        let nb = nb_api.get(&node.name_any()).await.unwrap();
        if nb.spec.asn != applied_cb.spec.asn_selector.asn.unwrap() {
            panic!("NodeBGP's ASN must be same as ClusterBGP's one")
        }
    }

    tracing::info!("Cleaning up a kind cluster");
    cleanup_kind();

    // Re create kind cluster for the other scenario

    tracing::info!("Setting up a kind cluster");
    setup_kind();

    tracing::info!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let ctx = State::default().to_context(client.clone(), 30, Arc::new(Mutex::new(Metrics::default())));

    let mut cb = test_cluster_bgp();
    cb.spec.asn_selector = AsnSelector {
        from: AsnSelectionType::Label,
        asn: None,
    };

    let cb_api = Api::<ClusterBGP>::all(ctx.client.clone());
    let ssapply = PatchParams::apply("ctrltest");
    let cb_patch = Patch::Apply(cb.clone());

    tracing::info!("Creating the ClusterBGP resource");
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

    // To use the resource that has uid, get it from the api server again after creating.
    // To set the owner reference, we need uid.
    let applied_cb = cb_api.get(&cb.name_any()).await.unwrap();

    // do reconcile
    tracing::info!("Reconciling the resouce when creating");
    controller::reconciler::cluster_bgp::reconciler(Arc::new(applied_cb.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Getting NodeBGP resources created by reconciling ClusterBGP");
    let node_api = Api::<Node>::all(ctx.client.clone());
    let nb_api = Api::<NodeBGP>::all(ctx.client.clone());
    let node_list = node_api.list(&ListParams::default()).await.unwrap();
    for node in node_list.iter() {
        let asn: u32 = node.labels().get(ASN_LABEL).unwrap().parse().unwrap();
        let nb = nb_api.get(&node.name_any()).await.unwrap();
        if nb.spec.asn != asn {
            panic!("NodeBGP's ASN must be same as ClusterBGP's one")
        }
    }

    tracing::info!("Cleaning up a kind cluster");
    cleanup_kind();
}
