use std::sync::{Arc, Mutex};

use futures::StreamExt;
use kube::{
    api::{ListParams, PostParams},
    runtime::{
        controller::Action,
        finalizer::{finalizer, Event},
        watcher::Config,
        Controller,
    },
    Api, Client, ResourceExt,
};
use tracing::{field, Span};

use crate::{
    controller::{
        context::{error_policy, Context, Ctx, State},
        error::Error,
        metrics::Metrics,
    },
    crd::bgp_advertisement::{AdvertiseStatus, BGPAdvertisement, BGP_ADVERTISEMENT_FINALIZER},
    util::get_namespace,
};

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(ba: Arc<BGPAdvertisement>, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<BGPAdvertisement>(&ba).map_err(Error::KubeLibrary)?;
    let bgp_advertisements = Api::<BGPAdvertisement>::namespaced(ctx.client.clone(), &ns);
    let metrics = ctx.metrics();
    metrics
        .lock()
        .map_err(|_| Error::FailedToGetLock)?
        .reconciliation(ba.as_ref());

    finalizer(
        &bgp_advertisements,
        BGP_ADVERTISEMENT_FINALIZER,
        ba,
        |event| async {
            match event {
                Event::Apply(ba) => reconcile(&bgp_advertisements, &ba, ctx).await,
                Event::Cleanup(ba) => cleanup(&bgp_advertisements, &ba, ctx).await,
            }
        },
    )
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(
    api: &Api<BGPAdvertisement>,
    ba: &BGPAdvertisement,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    let mut advertised = 0;
    let mut not_advertised = 0;
    let mut withdwraw = 0;
    if let Some(status) = ba.status.as_ref() {
        if let Some(peers) = status.peers.as_ref() {
            for (_peer, adv_status) in peers.iter() {
                match adv_status {
                    AdvertiseStatus::Advertised => advertised += 1,
                    AdvertiseStatus::NotAdvertised => not_advertised += 1,
                    AdvertiseStatus::Withdraw => withdwraw += 1,
                }
            }
        }
    }
    {
        let metrics = ctx.metrics();
        let metrics = metrics.lock().map_err(|_| Error::FailedToGetLock)?;
        metrics.bgp_advertisement_status_set(
            &ba.name_any(),
            &format!("{}", AdvertiseStatus::Advertised),
            advertised,
        );
        metrics.bgp_advertisement_status_set(
            &ba.name_any(),
            &format!("{}", AdvertiseStatus::NotAdvertised),
            not_advertised,
        );
        metrics.bgp_advertisement_status_set(
            &ba.name_any(),
            &format!("{}", AdvertiseStatus::Withdraw),
            withdwraw,
        );
    }

    let ba_list = api
        .list(&ListParams::default())
        .await
        .map_err(Error::Kube)?;
    let mut counter = 0;
    for b in ba_list.iter() {
        if b.spec.r#type.eq(&ba.spec.r#type) {
            counter += 1;
        }
    }
    ctx.metrics()
        .lock()
        .map_err(|_| Error::FailedToGetLock)?
        .bgp_advertisements_set(&format!("{}", ba.spec.r#type), counter as i64);
    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(
    api: &Api<BGPAdvertisement>,
    ba: &BGPAdvertisement,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    let trace_id = sartd_trace::telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));

    let ns = get_namespace::<BGPAdvertisement>(ba).map_err(Error::KubeLibrary)?;

    let mut new_ba = ba.clone();
    let mut need_update = false;
    if let Some(peers) = new_ba
        .status
        .as_mut()
        .and_then(|status| status.peers.as_mut())
    {
        if peers.is_empty() {
            tracing::info!(
                name = ba.name_any(),
                namespace = ns,
                "successfully delete BGPAdvertisement"
            );
            ctx.metrics()
                .lock()
                .map_err(|_| Error::FailedToGetLock)?
                .bgp_advertisements_dec(&format!("{}", ba.spec.r#type));
            return Ok(Action::await_change());
        }
        for (_p, s) in peers.iter_mut() {
            if s.ne(&&AdvertiseStatus::Withdraw) {
                *s = AdvertiseStatus::Withdraw;
                need_update = true;
            }
        }
    } else {
        tracing::info!(
            name = ba.name_any(),
            namespace = ns,
            "successfully delete BGPAdvertisement"
        );
        ctx.metrics()
            .lock()
            .map_err(|_| Error::FailedToGetLock)?
            .bgp_advertisements_dec(&format!("{}", ba.spec.r#type));
        return Ok(Action::await_change());
    }

    if need_update {
        api.replace_status(
            &ba.name_any(),
            &PostParams::default(),
            serde_json::to_vec(&new_ba).map_err(Error::Serialization)?,
        )
        .await
        .map_err(Error::Kube)?;

        tracing::info!(
            name = &ba.name_any(),
            namespace = ns,
            "submit withdraw request"
        );
    }

    // Avoid deleting target resource without withdrawing from all peers, return error.
    // I'm not sure to avoid deleting it without returning error
    Err(Error::Withdrawing)
}

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn run(state: State, interval: u64, metrics: Arc<Mutex<Metrics>>) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let bgp_advertisements = Api::<BGPAdvertisement>::all(client.clone());
    if let Err(e) = bgp_advertisements
        .list(&ListParams::default().limit(1))
        .await
    {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start BGPAdvertisement reconciler");

    // let node_name = std::env::var(ENV_HOSTNAME).expect("HOSTNAME environment value is not set");
    // let label_selector = format!("{}={}", LABEL_BGP_PEER_NODE, node_name);

    // let watch_config = Config::default().labels(&label_selector);
    let watch_config = Config::default();
    Controller::new(bgp_advertisements, watch_config.any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<BGPAdvertisement, Error, Context>,
            state.to_context(client, interval, metrics),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use assert_json_diff::assert_json_include;
    use chrono::Utc;
    use http::Response;
    use hyper::{body::to_bytes, Body};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;
    use kube::core::ObjectMeta;

    use crate::{
        controller::{context::Context, error::Error},
        crd::{
            address_pool::AddressType,
            bgp_advertisement::{
                AdvertiseStatus, BGPAdvertisement, BGPAdvertisementSpec, BGPAdvertisementStatus,
                Protocol, BGP_ADVERTISEMENT_FINALIZER,
            },
        },
        fixture::reconciler::{assert_resource_request, timeout_after_1s, ApiServerVerifier},
    };

    // BGPAdvertisement's reconciliation logic depends on finalizer handling.
    // So here, we tests with finalizer logic.
    use super::reconciler;

    enum Scenario {
        FinalizerCreation(BGPAdvertisement),
        CleanupFailByWithdrawing(BGPAdvertisement),
        CleanupNop(BGPAdvertisement),
        Cleanup(BGPAdvertisement),
    }

    impl ApiServerVerifier {
        fn controller_bgp_advertisement_run(
            self,
            scenario: Scenario,
        ) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async move {
                match scenario {
                    Scenario::FinalizerCreation(ba) => {
                        self.bgp_advertisement_create_finalizer(&ba).await
                    }
                    Scenario::CleanupFailByWithdrawing(ba) => {
                        self.bgp_advertisement_cleanup(&ba).await
                    }
                    Scenario::CleanupNop(ba) => self.bgp_advertisement_cleanup(&ba).await,
                    Scenario::Cleanup(ba) => {
                        self.bgp_advertisement_cleanup(&ba)
                            .await
                            .unwrap()
                            .bgp_advertisement_remove_finalizer(&ba)
                            .await
                    }
                }
                .expect("reconcile completed without error");
            })
        }

        async fn bgp_advertisement_create_finalizer(
            mut self,
            ba: &BGPAdvertisement,
        ) -> Result<Self, Error> {
            let (request, send) = self.0.next_request().await.expect("service not called");
            assert_resource_request(&request, ba, None, false, None, http::Method::PATCH);
            let expected_patch = serde_json::json!([
                { "op": "test", "path": "/metadata/finalizers", "value": null },
                { "op": "add", "path": "/metadata/finalizers", "value": vec![BGP_ADVERTISEMENT_FINALIZER] }
            ]);
            let req_body = to_bytes(request.into_body()).await.unwrap();
            let runtime_patch: serde_json::Value =
                serde_json::from_slice(&req_body).expect("valid document from runtime");
            assert_json_include!(actual: runtime_patch, expected: expected_patch);
            let mut new_ba = ba.clone();
            new_ba.metadata.finalizers = Some(vec![BGP_ADVERTISEMENT_FINALIZER.to_string()]);
            let response = serde_json::to_vec(&new_ba).unwrap(); // respond as the apiserver would have
            send.send_response(Response::builder().body(Body::from(response)).unwrap());
            Ok(self)
        }

        async fn bgp_advertisement_remove_finalizer(
            mut self,
            ba: &BGPAdvertisement,
        ) -> Result<Self, Error> {
            let (request, send) = self.0.next_request().await.expect("service not called");
            // We expect a json patch to the specified document removing our finalizer (at index 0)
            assert_resource_request(&request, ba, None, false, None, http::Method::PATCH);
            let expected_patch = serde_json::json!([
                { "op": "test", "path": "/metadata/finalizers/0", "value": BGP_ADVERTISEMENT_FINALIZER},
                { "op": "remove", "path": "/metadata/finalizers/0", "path": "/metadata/finalizers/0" }
            ]);
            let req_body = to_bytes(request.into_body()).await.unwrap();
            let runtime_patch: serde_json::Value =
                serde_json::from_slice(&req_body).expect("valid document from runtime");
            assert_json_include!(actual: runtime_patch, expected: expected_patch);

            let mut new_ba = ba.clone();
            new_ba.metadata.finalizers = None;
            let response = serde_json::to_vec(&new_ba).unwrap(); // respond as the apiserver would have
            send.send_response(Response::builder().body(Body::from(response)).unwrap());
            Ok(self)
        }

        async fn bgp_advertisement_cleanup(mut self, ba: &BGPAdvertisement) -> Result<Self, Error> {
            if ba
                .status
                .clone()
                .and_then(|status| {
                    status.peers.and_then(|peers| {
                        peers
                            .into_iter()
                            .find(|(_, p)| p.ne(&AdvertiseStatus::Withdraw))
                    })
                })
                .is_some()
            {
                let (request, send) = self.0.next_request().await.expect("service not called");
                assert_resource_request(
                    &request,
                    ba,
                    Some("status"),
                    false,
                    None,
                    http::Method::PUT,
                );
                let req_body = to_bytes(request.into_body()).await.unwrap();
                let json_val: serde_json::Value = serde_json::from_slice(&req_body).unwrap();
                let req_ba: BGPAdvertisement = serde_json::from_value(json_val).unwrap();
                if req_ba
                    .status
                    .as_ref()
                    .and_then(|status| {
                        status.peers.as_ref().and_then(|peer| {
                            peer.iter().find(|(_, p)| p.ne(&&AdvertiseStatus::Withdraw))
                        })
                    })
                    .is_some()
                {
                    panic!("requested BGPAdvertisement must not have any status entries that is not Withdraw status.");
                }
                send.send_response(
                    Response::builder()
                        .body(Body::from(serde_json::to_vec(&ba).unwrap()))
                        .unwrap(),
                );
            }
            Ok(self)
        }
    }

    #[tokio::test]
    async fn create_bgp_advertisement() {
        let (testctx, fakeserver, _) = Context::test();
        let ba = BGPAdvertisement {
            metadata: ObjectMeta {
                name: Some("test-adv1".to_string()),
                namespace: Some("default".to_string()),
                ..Default::default()
            },
            spec: BGPAdvertisementSpec {
                cidr: "10.0.0.1/32".to_string(),
                r#type: AddressType::Service,
                protocol: Protocol::IPv4,
                attrs: None,
            },
            status: None,
        };
        let mocksvr =
            fakeserver.controller_bgp_advertisement_run(Scenario::FinalizerCreation(ba.clone()));
        reconciler(Arc::new(ba), testctx).await.expect("reconciler");
        timeout_after_1s(mocksvr).await;
    }

    #[tokio::test]
    async fn cleanup_bgp_advertisement_fail_by_withdrawing() {
        let (testctx, fakeserver, _) = Context::test();
        let ba = BGPAdvertisement {
            metadata: ObjectMeta {
                name: Some("test-adv1".to_string()),
                namespace: Some("default".to_string()),
                finalizers: Some(vec![BGP_ADVERTISEMENT_FINALIZER.to_string()]),
                deletion_timestamp: Some(Time(Utc::now())),
                ..Default::default()
            },
            spec: BGPAdvertisementSpec {
                cidr: "10.0.0.1/32".to_string(),
                r#type: AddressType::Service,
                protocol: Protocol::IPv4,
                attrs: None,
            },
            status: Some(BGPAdvertisementStatus {
                peers: Some(BTreeMap::from([(
                    "test-peer".to_string(),
                    AdvertiseStatus::Advertised,
                )])),
            }),
        };
        let mocksvr = fakeserver
            .controller_bgp_advertisement_run(Scenario::CleanupFailByWithdrawing(ba.clone()));
        let res = if let Err(Error::Finalizer(e)) = reconciler(Arc::new(ba), testctx).await {
            e
        } else {
            panic!("reconciler should return Withdrawing error");
        };
        assert!(res.to_string().contains(&Error::Withdrawing.to_string()));
        timeout_after_1s(mocksvr).await;
    }

    #[tokio::test]
    async fn cleanup_bgp_advertisement_nop() {
        let (testctx, fakeserver, _) = Context::test();
        let ba = BGPAdvertisement {
            metadata: ObjectMeta {
                name: Some("test-adv1".to_string()),
                namespace: Some("default".to_string()),
                finalizers: Some(vec![BGP_ADVERTISEMENT_FINALIZER.to_string()]),
                deletion_timestamp: Some(Time(Utc::now())),
                ..Default::default()
            },
            spec: BGPAdvertisementSpec {
                cidr: "10.0.0.1/32".to_string(),
                r#type: AddressType::Service,
                protocol: Protocol::IPv4,
                attrs: None,
            },
            status: Some(BGPAdvertisementStatus {
                peers: Some(BTreeMap::from([(
                    "test-peer".to_string(),
                    AdvertiseStatus::Withdraw,
                )])),
            }),
        };
        let mocksvr = fakeserver.controller_bgp_advertisement_run(Scenario::CleanupNop(ba.clone()));
        let res = if let Err(Error::Finalizer(e)) = reconciler(Arc::new(ba), testctx).await {
            e
        } else {
            panic!("reconciler should return Withdrawing error");
        };
        assert!(res.to_string().contains(&Error::Withdrawing.to_string()));
        timeout_after_1s(mocksvr).await;
    }

    #[tokio::test]
    async fn cleanup_bgp_advertisement() {
        let (testctx, fakeserver, _) = Context::test();
        let ba = BGPAdvertisement {
            metadata: ObjectMeta {
                name: Some("test-adv1".to_string()),
                namespace: Some("default".to_string()),
                finalizers: Some(vec![BGP_ADVERTISEMENT_FINALIZER.to_string()]),
                deletion_timestamp: Some(Time(Utc::now())),
                ..Default::default()
            },
            spec: BGPAdvertisementSpec {
                cidr: "10.0.0.1/32".to_string(),
                r#type: AddressType::Service,
                protocol: Protocol::IPv4,
                attrs: None,
            },
            status: Some(BGPAdvertisementStatus {
                peers: Some(BTreeMap::new()),
            }),
        };
        let mocksvr = fakeserver.controller_bgp_advertisement_run(Scenario::Cleanup(ba.clone()));
        reconciler(Arc::new(ba), testctx).await.expect("reconciler");
        timeout_after_1s(mocksvr).await;
    }
}
