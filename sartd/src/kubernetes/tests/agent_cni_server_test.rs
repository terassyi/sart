use std::{net::IpAddr, str::FromStr, sync::{Arc, Mutex}, time::Duration};

use crate::common::{setup_kind, TestContainer, TestRoutingRule};

use ipnet::IpNet;
use k8s_openapi::api::core::v1::{Container, Node, Pod, PodSpec, ServiceAccount};
use kube::{
    api::{ListParams, ObjectMeta, Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_ipam::manager::{AllocatorSet, BlockAllocator};
use sartd_kubernetes::crd::address_block::{
    ADDRESS_BLOCK_FINALIZER_AGENT, ADDRESS_BLOCK_FINALIZER_CONTROLLER,
};
use sartd_kubernetes::{
    agent::{
        cni::server::{CNIServer, CNI_ROUTE_TABLE_ID},
        reconciler::address_block::{self, PodAllocator},
    },
    context::State,
    controller,
    crd::{
        address_block::AddressBlock,
        address_pool::AddressPool,
        block_request::{BlockRequest, BLOCK_REQUEST_FINALIZER},
    },
    fixture::{reconciler::test_address_pool_pod_another, test_trace},
};

use tokio::sync::mpsc::unbounded_channel;

mod common;

// This test is flaky...
// I'm not sure which line causes this issue.
#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_agent_cni_server() {
    // tracing::info!("Creating a kind cluster");
    // setup_kind();

    // test_trace().await;

    // tracing::info!("Getting kube client");
    // let client = Client::try_default().await.unwrap();
    // let block_allocator = Arc::new(BlockAllocator::default());
    // let ctx = State::default().to_context_with(client.clone(), 30, block_allocator);
    // let allocator_set = Arc::new(AllocatorSet::new());

    // let node_api = Api::<Node>::all(client.clone());
    // let node_list = node_api.list(&ListParams::default()).await.unwrap();
    // assert_eq!(node_list.items.len(), 1);
    // let node = node_list.items.first().unwrap();
    // let node_addr = IpAddr::from_str(
    //     &node
    //         .status
    //         .clone()
    //         .unwrap()
    //         .addresses
    //         .unwrap()
    //         .first()
    //         .unwrap()
    //         .address,
    // )
    // .unwrap();

    // let (sender, receiver) = unbounded_channel();

    // let cni_server = CNIServer::new(
    //     client.clone(),
    //     allocator_set.clone(),
    //     node.name_any(),
    //     node_addr,
    //     CNI_ROUTE_TABLE_ID,
    //     receiver,
    // );

    // let endpoint = "127.0.0.1:6789";

    // tracing::info!("Spawning CNI server");
    // tokio::spawn(async move {
    //     sartd_kubernetes::agent::cni::server::run(endpoint, cni_server).await;
    // });

    // tracing::info!("Waiting to run CNI server");
    // let mut cni_client = tokio::time::timeout(Duration::from_secs(60), async move {
    //     loop {
    //         if let Ok(client) = sartd_proto::sart::cni_api_client::CniApiClient::connect(format!(
    //             "http://{endpoint}"
    //         ))
    //         .await
    //         {
    //             break client;
    //         }
    //     }
    // })
    // .await
    // .unwrap();

    // // TestRoutingRule implements Drop trait to clean up routing rule in the kernel, when this test is finished.
    // let _rule4 = TestRoutingRule::new(CNI_ROUTE_TABLE_ID, false);
    // let _rule6 = TestRoutingRule::new(CNI_ROUTE_TABLE_ID, true);

    // let ap = test_address_pool_pod_another();

    // tracing::info!("Creating an AddressPool");
    // let address_pool_api = Api::<AddressPool>::all(client.clone());
    // let ssapply = PatchParams::apply("ctrltest");
    // let ap_patch = Patch::Apply(ap.clone());
    // address_pool_api
    //     .patch(&ap.name_any(), &ssapply, &ap_patch)
    //     .await
    //     .unwrap();

    // let applied_ap = address_pool_api.get(&ap.name_any()).await.unwrap();

    // tracing::info!("Reconciling AddressPool");
    // controller::reconciler::address_pool::reconciler(Arc::new(applied_ap.clone()), ctx.clone())
    //     .await
    //     .unwrap();

    // tracing::info!("Getting the applied pool");
    // let applied_ap = address_pool_api.get(&ap.name_any()).await.unwrap();
    // assert!(applied_ap.status.is_none());

    // tracing::info!("Waiting creating the service account in default namespace");
    // let service_account_api = Api::<ServiceAccount>::namespaced(client.clone(), "default");
    // tokio::time::timeout(Duration::from_secs(30), async move {
    //     let mut ticker = tokio::time::interval(Duration::from_secs(1));
    //     loop {
    //         ticker.tick().await;
    //         if let Ok(_sa) = service_account_api.get("default").await {
    //             break;
    //         }
    //     }
    // })
    // .await
    // .unwrap();

    // tracing::info!("Creating a dummy pod");
    // let pod1 = Pod {
    //     metadata: ObjectMeta {
    //         name: Some("pod1".to_string()),
    //         namespace: Some("default".to_string()),
    //         ..Default::default()
    //     },
    //     spec: Some(PodSpec {
    //         containers: vec![Container {
    //             image: Some("ghcr.io/terassyi/test-server:0.1".to_string()),
    //             name: "pod1".to_string(),
    //             ..Default::default()
    //         }],
    //         service_account_name: Some("default".to_string()),
    //         ..Default::default()
    //     }),
    //     status: None,
    // };
    // let pod_api = Api::<Pod>::namespaced(client.clone(), "default");
    // let pod1_patch = Patch::Apply(pod1.clone());
    // pod_api
    //     .patch(&pod1.name_any(), &ssapply, &pod1_patch)
    //     .await
    //     .unwrap();
    // let container1 = TestContainer::new(
    //     "1111111111111111",
    //     "/var/run/netns/pod1",
    //     "eth0",
    //     "opt/cni/bin/sart-cni",
    //     "pod1-uid",
    //     "pod1",
    //     "default",
    // );

    // tracing::info!("Preparing AddressBlock reconciler");
    // let pod_allocator = Arc::new(PodAllocator {
    //     allocator: allocator_set.clone(),
    //     notifier: sender.clone(),
    // });
    // let ab_ctx = State::default().to_context_with(client.clone(), 30, pod_allocator.clone());
    // let address_block_api = Api::<AddressBlock>::all(client.clone());

    // tracing::info!("Spawning BlockRequest reconciler");
    // let block_request_api = Api::<BlockRequest>::all(client.clone());
    // let block_request_api_cloned = block_request_api.clone();
    // let address_pool_api_cloned = address_pool_api.clone();
    // let ssapply_cloned = ssapply.clone();
    // tokio::spawn(async move {
    //     let mut br = tokio::time::timeout(Duration::from_secs(60), async move {
    //         loop {
    //             if let Ok(br) = block_request_api_cloned
    //                 .get("test-pool-sart-integration-control-plane")
    //                 .await
    //             {
    //                 break br;
    //             }
    //         }
    //     })
    //     .await
    //     .unwrap();
    //     br.finalizers_mut()
    //         .insert(0, BLOCK_REQUEST_FINALIZER.to_string());
    //     br.metadata.managed_fields = None;
    //     let br_patch = Patch::Apply(br.clone());
    //     let br_name = br.name_any();
    //     let block_request_api_cloned = block_request_api.clone();
    //     tokio::time::timeout(Duration::from_secs(60), async move {
    //         block_request_api_cloned
    //             .patch(&br_name, &ssapply_cloned, &br_patch)
    //             .await
    //             .unwrap();
    //     })
    //     .await
    //     .unwrap();
    //     // block_request_api
    //     //     .patch(&br.name_any(), &ssapply_cloned, &br_patch)
    //     //     .await
    //     //     .unwrap();
    //     let applied_br = block_request_api.get(&br.name_any()).await.unwrap();

    //     tracing::info!("Reconciling an BlockRequest");
    //     sartd_kubernetes::controller::reconciler::block_request::reconciler(
    //         Arc::new(applied_br),
    //         ctx.clone(),
    //     )
    //     .await
    //     .unwrap();

    //     tracing::info!("Reconciling an AddressPool to create new AddressBlock");
    //     let ap = address_pool_api_cloned.get("test-pool").await.unwrap();
    //     controller::reconciler::address_pool::reconciler(Arc::new(ap.clone()), ctx.clone())
    //         .await
    //         .unwrap();
    //     tracing::info!("Finish reconciling an AddressPool to create new AddressBlock");
    // });

    // tracing::info!("Spawning AddressBlock reconciler");
    // let address_block_api_cloned = address_block_api.clone();
    // let ssapply_cloned = ssapply.clone();
    // tokio::spawn(async move {
    //     let address_block_api_cloned2 = address_block_api_cloned.clone();
    //     let mut ab = tokio::time::timeout(Duration::from_secs(60), async move {
    //         loop {
    //             if let Ok(ba_list) = address_block_api_cloned2.list(&ListParams::default()).await {
    //                 if !ba_list.items.is_empty() {
    //                     break ba_list.items.first().unwrap().clone();
    //                 }
    //             }
    //         }
    //     })
    //     .await
    //     .unwrap();

    //     let ab_name = ab.name_any();
    //     let address_block_api_cloned3 = address_block_api_cloned.clone();
    //     tokio::time::timeout(Duration::from_secs(60), async move {
    //         loop {
    //             let mut ab = address_block_api_cloned3.get(&ab_name).await.unwrap();
    //             ab.finalizers_mut()
    //                 .insert(0, ADDRESS_BLOCK_FINALIZER_CONTROLLER.to_string());
    //             ab.finalizers_mut()
    //                 .insert(1, ADDRESS_BLOCK_FINALIZER_AGENT.to_string());
    //             tracing::info!(finalizer=?ab.finalizers(),"Finalizers");
    //             ab.metadata.managed_fields = None;
    //             let ab_patch = Patch::Apply(ab.clone());
    //             address_block_api_cloned3
    //                 .patch_metadata(&ab_name, &ssapply_cloned, &ab_patch)
    //                 .await
    //                 .unwrap();

    //             let applied_ab = address_block_api_cloned3.get(&ab_name).await.unwrap();
    //             if applied_ab.finalizers().len() != 2 {
    //                 tokio::time::sleep(Duration::from_secs(1)).await;
    //                 continue;
    //             }
    //         }
    //     })
    //     .await
    //     .unwrap();

    //     tracing::info!("Getting patched address block");
    //     let applied_ab = address_block_api_cloned.get(&ab.name_any()).await.unwrap();

    //     tracing::info!(finalizer=?applied_ab.finalizers(),"after Finalizers");
    //     tracing::info!("Reconciling an AddressBlock by agent");
    //     address_block::reconciler(Arc::new(applied_ab), ab_ctx.clone())
    //         .await
    //         .unwrap();
    //     // controller::reconciler::address_block::reconciler(applied_ab, ctx)
    // });
    // tracing::info!("Calling Add command by client");
    // let res = tokio::time::timeout(Duration::from_secs(60), async move {
    //     cni_client.add(container1.args.clone()).await.unwrap()
    // })
    // .await
    // .unwrap();
    // // let res = cni_client.add(container1.args.clone()).await.unwrap();

    // let resp = res.get_ref();

    // tracing::info!("Checking the response");
    // assert_eq!(resp.interfaces.len(), 1);
    // assert_eq!(resp.ips.len(), 1);
    // assert_eq!(resp.routes.len(), 1);

    // tracing::info!("Checking the allocation");
    // let pod_addr = IpNet::from_str(&resp.ips[0].address).unwrap();
    // {
    //     let tmp_allocator_set = allocator_set.clone();
    //     let tmp_allocator = tmp_allocator_set.inner.lock().unwrap();
    //     let block = tmp_allocator.blocks.get("test-pool-0").unwrap();
    //     assert!(block.allocator.is_allocated(&pod_addr.addr()));
    // }
}
