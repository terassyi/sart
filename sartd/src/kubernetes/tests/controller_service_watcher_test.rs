use std::{collections::HashMap, net::IpAddr, str::FromStr, sync::{Arc, Mutex}};

use common::{cleanup_kind, setup_kind};

use ipnet::IpNet;
use k8s_openapi::api::core::v1::Service;
use kube::{
    api::{DeleteParams, Patch, PatchParams},
    Api, Client, ResourceExt,
};
use sartd_ipam::manager::{AllocatorSet, Block};
use sartd_kubernetes::{
    controller::{
        self,
        context::{Ctx, State},
        metrics::Metrics,
        reconciler::service_watcher::{get_allocated_lb_addrs, RELEASE_ANNOTATION},
    },
    crd::address_pool::{ADDRESS_POOL_ANNOTATION, LOADBALANCER_ADDRESS_ANNOTATION},
    fixture::{
        reconciler::{test_svc, test_svc_with_name},
        test_trace,
    },
    util::get_namespace,
};

mod common;

#[tokio::test]
#[ignore = "use kind cluster"]
async fn integration_test_service_watcher() {
    tracing::info!("Setting up a kind cluster");
    setup_kind();

    test_trace().await;

    tracing::info!("Getting kube client");
    let client = Client::try_default().await.unwrap();
    let allocator_set = Arc::new(AllocatorSet::new());
    let ctx = State::default().to_context_with::<Arc<AllocatorSet>>(
        client.clone(),
        30,
        allocator_set.clone(),
        Arc::new(Mutex::new(Metrics::default())),
    );

    let pool_name = "test-pool";
    let block = Block::new(
        pool_name.to_string(),
        pool_name.to_string(),
        IpNet::from_str("10.0.0.0/16").unwrap(),
    )
    .unwrap();

    tracing::info!("Chencking the block is registered in allocator set");
    {
        let alloc_set = allocator_set.clone();
        let mut alloc_set_inner = alloc_set.inner.lock().unwrap();
        alloc_set_inner.insert(block, true).unwrap();
    }

    tracing::info!("Creating Service resource");
    let svc = test_svc();

    let ns = get_namespace(&svc).unwrap();
    let svc_api = Api::<Service>::namespaced(ctx.client().clone(), &ns);
    let ssapply = PatchParams::apply("ctrltest");
    let svc_patch = Patch::Apply(svc.clone());
    svc_api
        .patch(&svc.name_any(), &ssapply, &svc_patch)
        .await
        .unwrap();

    let applied_svc = svc_api.get(&svc.name_any()).await.unwrap();

    tracing::info!("Reconciling Service");
    controller::reconciler::service_watcher::reconciler(Arc::new(applied_svc.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Checking the allocated address");
    let allocated_svc = svc_api.get(&svc.name_any()).await.unwrap();
    let allocated_addr = get_allocated_lb_addrs(&allocated_svc).unwrap();
    assert_eq!(1, allocated_addr.len());
    assert_eq!(IpAddr::from_str("10.0.0.0").unwrap(), allocated_addr[0]);

    tracing::info!("Creating Service");
    let mut svc_require_addr = test_svc_with_name("test-svc-2");
    svc_require_addr.annotations_mut().insert(
        LOADBALANCER_ADDRESS_ANNOTATION.to_string(),
        "10.0.0.100".to_string(),
    );

    let svc_require_addr_patch = Patch::Apply(svc_require_addr.clone());
    svc_api
        .patch(
            &svc_require_addr.name_any(),
            &ssapply,
            &svc_require_addr_patch,
        )
        .await
        .unwrap();

    let applied_svc_require_addr = svc_api.get(&svc_require_addr.name_any()).await.unwrap();

    tracing::info!("Reconciling Service");
    controller::reconciler::service_watcher::reconciler(
        Arc::new(applied_svc_require_addr.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    tracing::info!("Checking the allocated address");
    let allocated_svc = svc_api.get(&svc_require_addr.name_any()).await.unwrap();
    let allocated_addr = get_allocated_lb_addrs(&allocated_svc).unwrap();
    assert_eq!(1, allocated_addr.len());
    assert_eq!(IpAddr::from_str("10.0.0.100").unwrap(), allocated_addr[0]);

    tracing::info!("Chencking the another block is registered in allocator set");
    let another_pool_name = "test-pool-another";
    let another_block = Block::new(
        another_pool_name.to_string(),
        another_pool_name.to_string(),
        IpNet::from_str("10.1.0.0/16").unwrap(),
    )
    .unwrap();
    {
        let alloc_set = allocator_set.clone();
        let mut alloc_set_inner = alloc_set.inner.lock().unwrap();
        alloc_set_inner.insert(another_block, false).unwrap();
    }

    tracing::info!("Creating Service");
    let mut svc_require_addr_another_pool = test_svc_with_name("test-svc-3");
    svc_require_addr_another_pool.annotations_mut().insert(
        ADDRESS_POOL_ANNOTATION.to_string(),
        another_pool_name.to_string(),
    );
    svc_require_addr_another_pool.annotations_mut().insert(
        LOADBALANCER_ADDRESS_ANNOTATION.to_string(),
        "10.1.0.101".to_string(),
    );

    let svc_require_addr_another_pool_patch = Patch::Apply(svc_require_addr_another_pool.clone());
    svc_api
        .patch(
            &svc_require_addr_another_pool.name_any(),
            &ssapply,
            &svc_require_addr_another_pool_patch,
        )
        .await
        .unwrap();

    let applied_svc_require_addr_another_pool = svc_api
        .get(&svc_require_addr_another_pool.name_any())
        .await
        .unwrap();

    tracing::info!("Reconciling Service");
    controller::reconciler::service_watcher::reconciler(
        Arc::new(applied_svc_require_addr_another_pool.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    tracing::info!("Checking the allocated address");
    let allocated_svc = svc_api
        .get(&svc_require_addr_another_pool.name_any())
        .await
        .unwrap();
    let allocated_addr = get_allocated_lb_addrs(&allocated_svc).unwrap();
    assert_eq!(1, allocated_addr.len());
    assert_eq!(IpAddr::from_str("10.1.0.101").unwrap(), allocated_addr[0]);

    tracing::info!("Creating Service");
    let mut svc_multi_pool = test_svc_with_name("test-svc-4");
    svc_multi_pool.annotations_mut().insert(
        ADDRESS_POOL_ANNOTATION.to_string(),
        format!("{},{}", pool_name, another_pool_name),
    );
    svc_multi_pool.annotations_mut().insert(
        LOADBALANCER_ADDRESS_ANNOTATION.to_string(),
        "10.1.0.102".to_string(),
    );

    let svc_multi_pool_patch = Patch::Apply(svc_multi_pool.clone());
    svc_api
        .patch(&svc_multi_pool.name_any(), &ssapply, &svc_multi_pool_patch)
        .await
        .unwrap();

    let applied_svc_multi_pool = svc_api.get(&svc_multi_pool.name_any()).await.unwrap();

    tracing::info!("Reconciling Service");
    controller::reconciler::service_watcher::reconciler(
        Arc::new(applied_svc_multi_pool.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    tracing::info!("Checking the allocated address");
    let allocated_svc = svc_api.get(&svc_multi_pool.name_any()).await.unwrap();
    let allocated_addr: HashMap<IpAddr, ()> = get_allocated_lb_addrs(&allocated_svc)
        .unwrap()
        .iter()
        .map(|a| (*a, ()))
        .collect();
    assert_eq!(2, allocated_addr.len());
    let expected = HashMap::from([
        (IpAddr::from_str("10.0.0.1").unwrap(), ()),
        (IpAddr::from_str("10.1.0.102").unwrap(), ()),
    ]);
    assert_eq!(expected, allocated_addr);

    tracing::info!("Updating Service with changing the address");

    let mut svc_update = svc.clone();
    svc_update.annotations_mut().insert(
        LOADBALANCER_ADDRESS_ANNOTATION.to_string(),
        "10.0.0.222".to_string(),
    );

    let svc_update_patch = Patch::Apply(svc_update.clone());
    svc_api
        .patch(&svc_update.name_any(), &ssapply, &svc_update_patch)
        .await
        .unwrap();

    tracing::info!("Reconciling Service");
    let applied_svc_update = svc_api.get(&svc_update.name_any()).await.unwrap();
    controller::reconciler::service_watcher::reconciler(
        Arc::new(applied_svc_update.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    tracing::info!("Checking the allocated address");
    let allocated_svc = svc_api.get(&svc_update.name_any()).await.unwrap();
    let allocated_addr = get_allocated_lb_addrs(&allocated_svc).unwrap();
    assert_eq!(1, allocated_addr.len());
    assert_eq!(IpAddr::from_str("10.0.0.222").unwrap(), allocated_addr[0]);

    tracing::info!("Updating Service with changing the pool");
    let mut svc_require_addr_another_pool_update = svc_require_addr_another_pool.clone();
    svc_require_addr_another_pool_update.metadata.annotations = None;
    let svc_require_addr_another_pool_update_patch =
        Patch::Apply(svc_require_addr_another_pool_update.clone());
    svc_api
        .patch(
            &svc_require_addr_another_pool_update.name_any(),
            &ssapply,
            &svc_require_addr_another_pool_update_patch,
        )
        .await
        .unwrap();

    tracing::info!("Reconciling Service");
    let applied_svc_require_addr_another_pool_update = svc_api
        .get(&svc_require_addr_another_pool_update.name_any())
        .await
        .unwrap();
    controller::reconciler::service_watcher::reconciler(
        Arc::new(applied_svc_require_addr_another_pool_update.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    tracing::info!("Checking the allocated address");
    let allocated_svc = svc_api
        .get(&svc_require_addr_another_pool_update.name_any())
        .await
        .unwrap();
    let allocated_addrs = get_allocated_lb_addrs(&allocated_svc).unwrap();
    assert_eq!(1, allocated_addrs.len());
    let test_svc3_addr = allocated_addrs[0];
    assert_eq!(IpAddr::from_str("10.0.0.0").unwrap(), test_svc3_addr);

    tracing::info!("Deleting Service");
    svc_api
        .delete(&svc_update.name_any(), &DeleteParams::default())
        .await
        .unwrap();

    tracing::info!("Cleaning up Service");
    let deleted_svc = svc_api.get(&svc_update.name_any()).await.unwrap();
    controller::reconciler::service_watcher::reconciler(Arc::new(deleted_svc.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Checking the allocation is deleted");
    let res = svc_api.get_opt(&deleted_svc.name_any()).await.unwrap();
    assert!(res.is_none());
    {
        let ai = allocator_set.clone();
        let alloc_set = ai.inner.lock().unwrap();
        let block = alloc_set.get(pool_name).unwrap();
        let res = block
            .allocator
            .is_allocated(&IpAddr::from_str("10.0.0.222").unwrap());
        assert!(!res);
    }

    tracing::info!("Deleting Service");
    svc_api
        .delete(&svc_multi_pool.name_any(), &DeleteParams::default())
        .await
        .unwrap();
    tracing::info!("Cleaning up Service");
    let deleted_svc = svc_api.get(&svc_multi_pool.name_any()).await.unwrap();
    controller::reconciler::service_watcher::reconciler(Arc::new(deleted_svc.clone()), ctx.clone())
        .await
        .unwrap();

    tracing::info!("Checking the allocation is deleted");
    let res = svc_api.get_opt(&deleted_svc.name_any()).await.unwrap();
    assert!(res.is_none());
    {
        let ai = allocator_set.clone();
        let alloc_set = ai.inner.lock().unwrap();
        let block = alloc_set.get(pool_name).unwrap();
        let res = block
            .allocator
            .is_allocated(&IpAddr::from_str("10.0.0.1").unwrap());
        assert!(!res);
        let block = alloc_set.get(another_pool_name).unwrap();
        let res = block
            .allocator
            .is_allocated(&IpAddr::from_str("10.0.0.102").unwrap());
        assert!(!res);
    }

    tracing::info!("Changing Service from LoadBalancer to ClusterIP");
    let mut svc_clusterip = svc_require_addr_another_pool_update.clone();
    svc_clusterip.spec.as_mut().unwrap().type_ = Some("ClusterIP".to_string());
    // After applying the update to the other type of service,
    // mutating webhook will add an annotation to notify releasable addresses to the reconciler.
    svc_clusterip
        .annotations_mut()
        .insert(RELEASE_ANNOTATION.to_string(), "10.0.0.0".to_string());
    let svc_clusterip_patch = Patch::Apply(svc_clusterip.clone());

    svc_api
        .patch(&svc_clusterip.name_any(), &ssapply, &svc_clusterip_patch)
        .await
        .unwrap();

    tracing::info!("Checking existence");
    let applied_svc_clusterip = svc_api.get(&svc_clusterip.name_any()).await.unwrap();
    assert_eq!(
        "ClusterIP",
        applied_svc_clusterip
            .spec
            .as_ref()
            .unwrap()
            .type_
            .as_ref()
            .unwrap()
    );

    tracing::info!("Reconciling Service");
    controller::reconciler::service_watcher::reconciler(
        Arc::new(applied_svc_clusterip.clone()),
        ctx.clone(),
    )
    .await
    .unwrap();

    std::thread::sleep(std::time::Duration::from_secs(2));
    tracing::info!("Checking the allocation is deleted");
    {
        let ai = allocator_set.clone();
        let alloc_set = ai.inner.lock().unwrap();
        let block = alloc_set.get(pool_name).unwrap();
        let res = block.allocator.is_allocated(&test_svc3_addr);
        assert!(!res);
    }

    tracing::info!("Cleaning up a kind cluster");
    cleanup_kind();
}
