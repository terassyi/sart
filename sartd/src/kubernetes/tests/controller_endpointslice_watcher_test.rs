use std::sync::Arc;

use k8s_openapi::api::{
    core::v1::Service,
    discovery::v1::{Endpoint, EndpointConditions, EndpointSlice},
};
use kube::{
    api::{DeleteParams, ListParams, Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_kubernetes::{
    context::{Context, State},
    controller::{self, reconciler::service_watcher::SERVICE_ETP_ANNOTATION},
    crd::{
        bgp_advertisement::{AdvertiseStatus, BGPAdvertisement},
        bgp_peer::BGPPeer,
        node_bgp::NodeBGP,
    },
    fixture::{
        reconciler::{test_bgp_peers, test_eps, test_node_bgp_list, test_svc, TestBgpSelector},
        test_trace,
    },
    util::get_namespace,
};

use crate::common::{cleanup_kind, setup_kind};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_endpointslice_watcher() {
    dbg!("Setting up a kind cluster");
    setup_kind();

    test_trace().await;

    dbg!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let ctx = State::default().to_context(client.clone(), 30);

    let eps = test_eps();

    let ssapply = PatchParams::apply("ctrltest");
    let ns = get_namespace(&eps).unwrap();

    dbg!("Preparing needed resources");
    prepare(&ctx, &ssapply, &ns).await;

    let eps_api = Api::<EndpointSlice>::namespaced(ctx.client.clone(), &ns);

    let eps_patch = Patch::Apply(eps.clone());
    eps_api
        .patch(&eps.name_any(), &ssapply, &eps_patch)
        .await
        .unwrap();

    let applied_eps = eps_api.get(&eps.name_any()).await.unwrap();

    dbg!("Reconciling EndpointSlice");
    controller::reconciler::endpointslice_watcher::reconciler(
        Arc::new(applied_eps.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    dbg!("Getting BGPAdvertisement");
    let ba_api = Api::<BGPAdvertisement>::namespaced(ctx.client.clone(), &ns);
    let bas = ba_api.list(&ListParams::default()).await.unwrap();
    assert_eq!(1, bas.items.len());

    let ba = bas.items[0].clone();

    dbg!("Reconciling EndpointSlice again because of requeue");
    controller::reconciler::endpointslice_watcher::reconciler(
        Arc::new(applied_eps.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    dbg!("Checking the BGPAdvertisement's status");
    let ba = ba_api.get(&ba.name_any()).await.unwrap();
    // exptected externalTrafficPolicy is Cluster, so the advertisement must have all peers for targets.
    assert_eq!(4, ba.status.as_ref().unwrap().peers.as_ref().unwrap().len());

    dbg!("Changing externalTrafficPolicy");
    // for notifying externalTrafficPolicy's change to endpointsliece_watcher,
    // update an annoation when owning LoadBalancer's externalTrafficPolicy is changed.
    let mut svc = test_svc();
    svc.spec.as_mut().unwrap().external_traffic_policy = Some("Local".to_string());
    let svc_patch = Patch::Apply(svc.clone());

    let svc_api = Api::<Service>::namespaced(ctx.client.clone(), &ns);
    svc_api
        .patch(&svc.name_any(), &ssapply, &svc_patch)
        .await
        .unwrap();

    let mut eps_with_local = eps.clone();
    eps_with_local
        .annotations_mut()
        .insert(SERVICE_ETP_ANNOTATION.to_string(), "Local".to_string());

    let eps_with_local_patch = Patch::Apply(eps_with_local.clone());
    eps_api
        .patch(&eps_with_local.name_any(), &ssapply, &eps_with_local_patch)
        .await
        .unwrap();

    let applied_eps = eps_api.get(&eps_with_local.name_any()).await.unwrap();

    dbg!("Reconciling EndpointSlice");
    controller::reconciler::endpointslice_watcher::reconciler(
        Arc::new(applied_eps.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    dbg!("Checking the BGPAdvertisement's status");
    let ba = ba_api.get(&ba.name_any()).await.unwrap();
    // expected externalTrafficPolicy is Cluster, so the advertisement must have all peers for targets.
    assert_eq!(4, ba.status.as_ref().unwrap().peers.as_ref().unwrap().len());
    let a = ba
        .status
        .as_ref()
        .unwrap()
        .peers
        .as_ref()
        .unwrap()
        .get("test1-peer1")
        .unwrap();
    assert_eq!(AdvertiseStatus::NotAdvertised, *a);
    let a = ba
        .status
        .as_ref()
        .unwrap()
        .peers
        .as_ref()
        .unwrap()
        .get("test3-peer1")
        .unwrap();
    assert_eq!(AdvertiseStatus::Withdraw, *a);

    dbg!("Updating endpoints");
    let mut eps_update = eps_with_local.clone();
    let new_ep = vec![
        Endpoint {
            addresses: vec!["192.168.0.1".to_string()],
            conditions: Some(EndpointConditions {
                ready: Some(true),
                serving: Some(true),
                terminating: Some(false),
            }),
            node_name: Some("test1".to_string()),
            ..Default::default()
        },
        Endpoint {
            addresses: vec!["192.168.0.2".to_string()],
            conditions: Some(EndpointConditions {
                ready: Some(false),
                serving: Some(false),
                terminating: Some(true),
            }),
            node_name: Some("test1".to_string()),
            ..Default::default()
        },
        Endpoint {
            addresses: vec!["192.168.0.2".to_string()],
            conditions: Some(EndpointConditions {
                ready: Some(true),
                serving: Some(true),
                terminating: Some(false),
            }),
            node_name: Some("test3".to_string()),
            ..Default::default()
        },
    ];
    eps_update.endpoints = new_ep;

    let eps_update_patch = Patch::Apply(eps_update.clone());
    eps_api
        .patch(&eps_update.name_any(), &ssapply, &eps_update_patch)
        .await
        .unwrap();

    let applied_eps = eps_api.get(&eps_update.name_any()).await.unwrap();

    dbg!("Reconciling EndpointSlice");
    controller::reconciler::endpointslice_watcher::reconciler(
        Arc::new(applied_eps.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    dbg!("Checking the BGPAdvertisement's status");
    let ba = ba_api.get(&ba.name_any()).await.unwrap();
    // expected externalTrafficPolicy is Cluster, so the advertisement must have all peers for targets.
    assert_eq!(4, ba.status.as_ref().unwrap().peers.as_ref().unwrap().len());
    let a = ba
        .status
        .as_ref()
        .unwrap()
        .peers
        .as_ref()
        .unwrap()
        .get("test1-peer1")
        .unwrap();
    assert_eq!(AdvertiseStatus::NotAdvertised, *a);
    let a = ba
        .status
        .as_ref()
        .unwrap()
        .peers
        .as_ref()
        .unwrap()
        .get("test2-peer1")
        .unwrap();
    assert_eq!(AdvertiseStatus::Withdraw, *a);
    let a = ba
        .status
        .as_ref()
        .unwrap()
        .peers
        .as_ref()
        .unwrap()
        .get("test3-peer1")
        .unwrap();
    assert_eq!(AdvertiseStatus::NotAdvertised, *a);

    dbg!("Deleting EndpointSlice");
    eps_api
        .delete(&eps_update.name_any(), &DeleteParams::default())
        .await
        .unwrap();

    dbg!("Cleaning up EndpointSlice");
    let deleted_eps = eps_api.get(&eps_update.name_any()).await.unwrap();
    controller::reconciler::endpointslice_watcher::reconciler(
        Arc::new(deleted_eps.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    dbg!("Cleaning up a kind cluster");
    cleanup_kind();
}

async fn prepare(ctx: &Arc<Context>, ssapply: &PatchParams, ns: &str) {
    let nbs = test_node_bgp_list(TestBgpSelector::All);
    let nb_api = Api::<NodeBGP>::all(ctx.client.clone());
    for nb in nbs.iter() {
        let patch = Patch::Apply(nb.clone());
        nb_api.patch(&nb.name_any(), ssapply, &patch).await.unwrap();
    }

    let bps = test_bgp_peers();
    let bp_api = Api::<BGPPeer>::all(ctx.client.clone());
    for bp in bps.iter() {
        let patch = Patch::Apply(bp.clone());
        bp_api.patch(&bp.name_any(), ssapply, &patch).await.unwrap();
    }

    let svc = test_svc();
    let svc_patch = Patch::Apply(svc.clone());

    let svc_api = Api::<Service>::namespaced(ctx.client.clone(), ns);
    svc_api
        .patch(&svc.name_any(), ssapply, &svc_patch)
        .await
        .unwrap();

    svc_api
        .patch_status(&svc.name_any(), ssapply, &svc_patch)
        .await
        .unwrap();
}
