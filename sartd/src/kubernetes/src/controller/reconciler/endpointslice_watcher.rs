use std::{
    collections::{BTreeMap, HashMap},
    net::IpAddr,
    sync::Arc,
    time::Duration,
};

use futures::StreamExt;
use ipnet::IpNet;
use k8s_openapi::api::{core::v1::Service, discovery::v1::EndpointSlice};
use kube::{
    api::{ListParams, PostParams, DeleteParams},
    core::ObjectMeta,
    runtime::{
        controller::Action,
        finalizer::{finalizer, Event},
        watcher::Config,
        Controller,
    },
    Api, Client, ResourceExt,
};

use crate::{
    context::{error_policy, Context, Ctx, State},
    controller::{
        error::Error,
        reconciler::service_watcher::{get_allocated_lb_addrs, is_loadbalancer},
    },
    crd::{
        address_pool::AddressType,
        bgp_advertisement::{
            AdvertiseStatus, BGPAdvertisement, BGPAdvertisementSpec, BGPAdvertisementStatus,
            Protocol,
        },
        bgp_peer::PEER_GROUP_ANNOTATION,
        node_bgp::NodeBGP,
    },
    util::{create_owner_reference, get_namespace},
};

use super::service_watcher::SERVICE_NAME_LABEL;

pub const ENDPOINTSLICE_FINALIZER: &str = "endpointslice.sart.terassyi.net/finalizer";

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(eps: Arc<EndpointSlice>, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<EndpointSlice>(&eps).map_err(Error::KubeLibrary)?;

    let endpointslices = Api::<EndpointSlice>::namespaced(ctx.client().clone(), &ns);

    finalizer(
        &endpointslices,
        ENDPOINTSLICE_FINALIZER,
        eps,
        |event| async {
            match event {
                Event::Apply(eps) => reconcile(&eps, ctx.clone()).await,
                Event::Cleanup(eps) => cleanup(&eps, ctx.clone()).await,
            }
        },
    )
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(eps: &EndpointSlice, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<EndpointSlice>(eps).map_err(Error::KubeLibrary)?;
    tracing::info!(
        name = eps.name_any(),
        namespace = ns,
        "Reconcile Endpointslice"
    );

    let svc_name = match get_svc_name_from_eps(eps) {
        Some(n) => n,
        None => return Ok(Action::await_change()),
    };
    let services = Api::<Service>::namespaced(ctx.client().clone(), &ns);
    let svc = match services.get_opt(svc_name).await.map_err(Error::Kube)? {
        Some(svc) => svc,
        None => {
            tracing::warn!(
                name = svc_name,
                "The Service resource associated with EndpointSlice is not found"
            );
            return Ok(Action::await_change());
        }
    };

    if !is_loadbalancer(&svc) {
        return Ok(Action::await_change());
    }

    let lb_addrs = match get_allocated_lb_addrs(&svc) {
        Some(lb_addrs) => lb_addrs,
        None => {
            tracing::warn!(
                name = svc.name_any(),
                namespace = ns,
                "LoadBalancer address is not allocated yet. Reconcile after 10 seconds"
            );
            return Ok(Action::requeue(Duration::from_secs(10)));
        }
    };

    let node_bgps = Api::<NodeBGP>::all(ctx.client().clone());
    let target_peers = get_target_peers(eps, &svc, &node_bgps).await?;

    let bgp_advertisements = Api::<BGPAdvertisement>::namespaced(ctx.client().clone(), ns.as_str());
    
    let label = format!("{}={}", SERVICE_NAME_LABEL, svc_name);
    let adv_list = bgp_advertisements.list(&ListParams::default().labels(&label)).await.map_err(Error::Kube)?;
    let mut existing_adv_map: HashMap<String, BGPAdvertisement> = adv_list.into_iter().map(|a| (a.spec.cidr.clone(), a)).collect();

    // If Endpointslice has no endpoint, requeue and wait for creating endpoints
    // In case of creating new BGPAdvertisement, there are the case that endpointslice has no valid endpoint
    let mut need_requeue = target_peers.is_empty();
    for addr in lb_addrs.iter() {
        let cidr = IpNet::new(*addr, 32).map_err(|_| Error::InvalidAddress)?;
        let cidr_str = cidr.to_string();
        let adv_name = adv_name_from_eps_and_addr(eps, addr);

        // get from existing advertisement list
        // and remove from its map
        match existing_adv_map.remove(&cidr_str)
        {
            Some(adv) => {
                let mut new_adv = adv.clone();

                if adv.spec.cidr.ne(cidr.to_string().as_str()) {
                    tracing::warn!(
                        name = svc.name_any(),
                        namespace = ns,
                        "LoadBalancer address is changed"
                    );
                    new_adv.spec.cidr = cidr.to_string();
                    let mut new_target_peers: BTreeMap<String, AdvertiseStatus> = BTreeMap::new();
                    if let Some(target_peers) = new_adv.status.and_then(|target| target.peers) {
                        for (target, _status) in target_peers.iter() {
                            new_target_peers.insert(target.clone(), AdvertiseStatus::NotAdvertised);
                        }
                    }
                    new_adv.status = Some(BGPAdvertisementStatus {
                        peers: Some(new_target_peers),
                    });
                    bgp_advertisements
                        .replace_status(
                            &new_adv.name_any(),
                            &PostParams::default(),
                            serde_json::to_vec(&new_adv).map_err(Error::Serialization)?,
                        )
                        .await
                        .map_err(Error::Kube)?;
                }

                let need_update = match new_adv
                    .status
                    .as_mut()
                    .and_then(|status| status.peers.as_mut())
                {
                    Some(peers) => sync_target_peers(peers, &target_peers),
                    None => {
                        let mut peers: BTreeMap<String, AdvertiseStatus> = BTreeMap::new();
                        for target in target_peers.iter() {
                            peers.insert(target.clone(), AdvertiseStatus::NotAdvertised);
                        }
                        new_adv.status = Some(BGPAdvertisementStatus { peers: Some(peers) });
                        true
                    }
                };

                tracing::info!(name = adv.name_any(), namespace = ns, targets =? target_peers, status =? new_adv.status, "Sync target peers");

                if need_update {
                    bgp_advertisements
                        .replace_status(
                            &new_adv.name_any(),
                            &PostParams::default(),
                            serde_json::to_vec(&new_adv).map_err(Error::Serialization)?,
                        )
                        .await
                        .map_err(Error::Kube)?;
                }
            }
            None => {
                // create new Advertisement
                let mut peers = BTreeMap::new();
                for p in target_peers.iter() {
                    peers.insert(p.clone(), AdvertiseStatus::NotAdvertised);
                }

                let adv = BGPAdvertisement {
                    metadata: ObjectMeta {
                        name: Some(adv_name),
                        namespace: eps.namespace(),
                        labels: Some(BTreeMap::from([(SERVICE_NAME_LABEL.to_string(), svc_name.to_string())])),
                        owner_references: Some(vec![create_owner_reference(eps)]),
                        ..Default::default()
                    },
                    spec: BGPAdvertisementSpec {
                        cidr: cidr.to_string(),
                        r#type: AddressType::Service,
                        protocol: Protocol::from(&cidr),
                        attrs: None, // TODO: implement BGP attributes
                    },
                    status: Some(BGPAdvertisementStatus { peers: Some(peers) }),
                };

                tracing::info!(
                    name = adv.name_any(),
                    namespace = ns,
                    status = ?adv.status,
                    "Create BGPAdvertisement"
                );

                bgp_advertisements
                    .create(&PostParams::default(), &adv)
                    .await
                    .map_err(Error::Kube)?;

                need_requeue = true;
            }
        }
    }
    // After handling advertisements related to actually allocated addresses,
    for (_, removable) in existing_adv_map.iter() {
        bgp_advertisements.delete(&removable.name_any(), &DeleteParams::default()).await.map_err(Error::Kube)?;
    }

    if need_requeue {
        Ok(Action::requeue(Duration::from_secs(10)))
    } else {
        Ok(Action::await_change())
    }
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(eps: &EndpointSlice, _ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<EndpointSlice>(eps).map_err(Error::KubeLibrary)?;

    tracing::info!(
        name = eps.name_any(),
        namespace = ns,
        "Cleanup Endpointslice"
    );

    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let endpointslices = Api::<EndpointSlice>::all(client.clone());

    tracing::info!("Start Endpointslice watcher");

    Controller::new(endpointslices, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<EndpointSlice, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

#[tracing::instrument(skip_all)]
async fn get_target_peers(
    eps: &EndpointSlice,
    svc: &Service,
    nb_api: &Api<NodeBGP>,
) -> Result<Vec<String>, Error> {
    let ns = match eps.namespace() {
        Some(ns) => ns,
        None => {
            tracing::warn!(
                name = eps.name_any(),
                "Namespace is not set in EndpointSlice"
            );
            return Err(Error::FailedToGetData("Namespace is required".to_string()));
        }
    };
    let mut target_peers = Vec::new();

    if let Some(svc_spec) = &svc.spec {
        if let Some(etp) = &svc_spec.external_traffic_policy {
            match etp.as_str() {
                "Cluster" => {
                    let nb_lists = nb_api
                        .list(&ListParams::default())
                        .await
                        .map_err(Error::Kube)?;
                    for nb in nb_lists.items.iter() {
                        let groups = get_svc_peer_groups(svc);
                        if let Some(peers) = &nb.spec.peers {
                            for peer in peers.iter() {
                                if peer.match_groups(&groups) {
                                    target_peers.push(peer.name.clone())
                                }
                            }
                        }
                    }
                }
                "Local" => {
                    for ep in eps.endpoints.iter() {
                        // Check wether its endpoint is available(serving?? or ready??)
                        // The lifecycle of kubernetes endpoints
                        // ref: https://kubernetes.io/docs/tutorials/services/pods-and-endpoint-termination-flow/
                        let cond = match ep.conditions.as_ref() {
                            Some(cond) => cond,
                            None => continue,
                        };
                        if !cond.serving.unwrap_or(false) {
                            tracing::warn!(name = svc.name_any(), namespace = ns, endpoints=?ep.addresses, "Endpoint is not serving");
                            continue;
                        }
                        if let Some(node_name) = &ep.node_name {
                            match nb_api.get_opt(node_name).await.map_err(Error::Kube)? {
                                Some(nb) => {
                                    // Compare labels the Service has and labels each NodeBGP has
                                    //
                                    let groups = get_svc_peer_groups(svc);
                                    if let Some(peers) = nb.spec.peers {
                                        for peer in peers.iter() {
                                            if peer.match_groups(&groups) {
                                                target_peers.push(peer.name.clone())
                                            }
                                        }
                                    }
                                }
                                None => tracing::warn!(
                                    name = svc.name_any(),
                                    namespace = ns,
                                    node = node_name,
                                    "There is no NodeBGP resource to the endpoint node"
                                ),
                            }
                        }
                    }
                }
                _ => return Err(Error::InvalidExternalTrafficPolicy),
            }
        }
    }

    target_peers.sort();
    target_peers.dedup();

    Ok(target_peers)
}

fn get_svc_name_from_eps(eps: &EndpointSlice) -> Option<&String> {
    eps.labels().get(SERVICE_NAME_LABEL)
}

fn get_svc_peer_groups(svc: &Service) -> Vec<&str> {
    match &svc.annotations().get(PEER_GROUP_ANNOTATION) {
        Some(v) => v.split(',').collect(),
        None => vec![],
    }
}

fn sync_target_peers(peers: &mut BTreeMap<String, AdvertiseStatus>, targets: &[String]) -> bool {
    let mut target_map: HashMap<&str, ()> = HashMap::new();
    let mut updated = false;
    for target in targets.iter() {
        target_map.insert(target, ());
        match peers.get(target) {
            Some(p) => {
                if p.eq(&AdvertiseStatus::Withdraw) {
                    peers.insert(target.clone(), AdvertiseStatus::NotAdvertised);
                    updated = true;
                }
            }
            None => {
                peers.insert(target.clone(), AdvertiseStatus::NotAdvertised);
                updated = true;
            }
        }
    }
    for (k, v) in peers.iter_mut() {
        if target_map.get(k.as_str()).is_none() && (*v).ne(&AdvertiseStatus::Withdraw) {
            *v = AdvertiseStatus::Withdraw;
            updated = true;
        }
    }
    updated
}

fn adv_name_from_eps_and_addr(eps: &EndpointSlice, addr: &IpAddr) -> String {
    format!("{}-{}", eps.name_any(), addr)
}

#[cfg(test)]
mod tests {

    use crate::fixture::reconciler::{
        assert_resource_request,
        test_eps, test_svc, timeout_after_1s, ApiServerVerifier
    };

    use super::*;

    use http::Response;
    use hyper::{body::to_bytes, Body};
    use k8s_openapi::api::core::v1::ServiceStatus;
    use rstest::rstest;

    #[rstest(
        peers,
        targets,
        updated,
        expected,
        case(BTreeMap::from([]), vec![], false, BTreeMap::from([])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised)]), vec!["peer1".to_string()], false, BTreeMap::from([])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised), ("peer2".to_string(), AdvertiseStatus::Advertised)]), vec!["peer1".to_string(), "peer2".to_string()], false, BTreeMap::from([])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised), ("peer2".to_string(), AdvertiseStatus::NotAdvertised)]), vec!["peer1".to_string(), "peer2".to_string()], false, BTreeMap::from([])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised)]), vec!["peer1".to_string(), "peer2".to_string()], true, BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised), ("peer2".to_string(), AdvertiseStatus::NotAdvertised)])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised), ("peer2".to_string(), AdvertiseStatus::Advertised)]), vec!["peer2".to_string()], true, BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Withdraw), ("peer2".to_string(), AdvertiseStatus::Advertised)])),
        case(BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Advertised), ("peer2".to_string(), AdvertiseStatus::Advertised)]), vec!["peer2".to_string(), "peer3".to_string()], true, BTreeMap::from([("peer1".to_string(), AdvertiseStatus::Withdraw), ("peer2".to_string(), AdvertiseStatus::Advertised), ("peer3".to_string(), AdvertiseStatus::NotAdvertised)])),
    )]
    fn works_sync_target_peers(
        mut peers: BTreeMap<String, AdvertiseStatus>,
        targets: Vec<String>,
        updated: bool,
        expected: BTreeMap<String, AdvertiseStatus>,
    ) {
        let res = sync_target_peers(&mut peers, &targets);
        assert_eq!(res, updated);
        if res {
            assert_eq!(peers, expected);
        }
    }

    enum Scenario {
        CreateNoLB(EndpointSlice),
        CreateNoAllocatedAddress(EndpointSlice),
    }

    impl ApiServerVerifier {
        fn endpointslice_run(self, scenario: Scenario) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async move {
                match scenario {
                    Scenario::CreateNoLB(eps) => self.endpointslice_create_not_lb(&eps).await,
                    Scenario::CreateNoAllocatedAddress(eps) => {
                        self.endpointslice_create_no_alloced(&eps).await
                    }
                }
                .expect("reconcile completed without error");
            })
        }

        async fn endpointslice_create_not_lb(
            mut self,
            _eps: &EndpointSlice,
        ) -> Result<Self, Error> {
            let (request, send) = self.0.next_request().await.expect("service not called");
            let mut svc = test_svc();
            svc.spec.as_mut().unwrap().type_ = Some("ClusterIP".to_string());

            assert_resource_request(&request, &svc, None, false, None, http::Method::GET);
            send.send_response(
                Response::builder()
                    .body(Body::from(serde_json::to_vec(&svc).unwrap()))
                    .unwrap(),
            );
            Ok(self)
        }

        async fn endpointslice_create_no_alloced(
            mut self,
            _eps: &EndpointSlice,
        ) -> Result<Self, Error> {
            let (request, send) = self.0.next_request().await.expect("service not called");
            let mut svc = test_svc();
            *svc.status.as_mut().unwrap() = ServiceStatus::default();

            assert_resource_request(&request, &svc, None, false, None, http::Method::GET);
            send.send_response(
                Response::builder()
                    .body(Body::from(serde_json::to_vec(&svc).unwrap()))
                    .unwrap(),
            );
            Ok(self)
        }
    }

    #[tokio::test]
    async fn endpointslice_create_not_lb() {
        let (testctx, fakeserver, _) = Context::test();
        let eps = test_eps();

        let mocksvr = fakeserver.endpointslice_run(Scenario::CreateNoLB(eps.clone()));
        reconcile(&eps, testctx).await.expect("reconciler");
        timeout_after_1s(mocksvr).await;
    }

    #[tokio::test]
    async fn endpointslice_create_no_alloced() {
        let (testctx, fakeserver, _) = Context::test();
        let eps = test_eps();

        let mocksvr = fakeserver.endpointslice_run(Scenario::CreateNoAllocatedAddress(eps.clone()));
        reconcile(&eps, testctx).await.expect("reconciler");
        timeout_after_1s(mocksvr).await;
    }
}
