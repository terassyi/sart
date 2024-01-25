use std::sync::Arc;

use futures::StreamExt;
use kube::{
    api::{ListParams, PostParams},
    core::ObjectMeta,
    runtime::{
        controller::{Action, Controller},
        finalizer::{finalizer, Event},
        watcher::Config,
    },
    Api, Client, ResourceExt,
};

use crate::{
    context::{error_policy, Context, State},
    controller::error::Error,
    crd::{
        address_block::{AddressBlock, AddressBlockSpec},
        address_pool::{AddressPool, AddressPoolStatus, AddressType, ADDRESS_POOL_FINALIZER},
    },
    util::create_owner_reference,
};

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(ap: Arc<AddressPool>, ctx: Arc<Context>) -> Result<Action, Error> {
    let address_pools = Api::<AddressPool>::all(ctx.client.clone());

    finalizer(&address_pools, ADDRESS_POOL_FINALIZER, ap, |event| async {
        match event {
            Event::Apply(ap) => reconcile(&address_pools, &ap, ctx.clone()).await,
            Event::Cleanup(ap) => cleanup(&address_pools, &ap, ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all)]
async fn reconcile(
    api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    tracing::info!(name = ap.name_any(), "reconcile AddressPool");

    match ap.spec.r#type {
        AddressType::Service => reconcile_service_pool(api, ap, ctx).await,
        AddressType::Pod => reconcile_pod_pool(api, ap, ctx).await,
    }
}

#[tracing::instrument(skip_all)]
async fn reconcile_service_pool(
    api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    let address_blocks = Api::<AddressBlock>::all(ctx.client.clone());

    match address_blocks
        .get_opt(&ap.name_any())
        .await
        .map_err(Error::Kube)?
    {
        Some(ab) => {
            tracing::warn!(name = ab.name_any(), "AddressBlock already exists");
        }
        None => {
            let ab = AddressBlock {
                metadata: ObjectMeta {
                    name: Some(ap.name_any()),
                    owner_references: Some(vec![create_owner_reference(ap)]),
                    ..Default::default()
                },
                spec: AddressBlockSpec {
                    cidr: ap.spec.cidr.clone(),
                    r#type: ap.spec.r#type,
                    pool_ref: ap.name_any(),
                    node_ref: None,
                    auto_assign: ap.spec.auto_assign.unwrap_or(false),
                },
                status: None,
            };

            address_blocks
                .create(&PostParams::default(), &ab)
                .await
                .map_err(Error::Kube)?;
        }
    }

    let mut new_ap = ap.clone();
    let mut need_update = false;
    match new_ap
        .status
        .as_mut()
        .and_then(|status| status.allocated.as_mut())
    {
        Some(allocated) => {
            if !allocated.contains(&ap.name_any()) {
                allocated.push(ap.name_any());
                need_update = true;
            }
        }
        None => {
            new_ap.status = Some(AddressPoolStatus {
                requested: None,
                allocated: Some(vec![ap.name_any()]),
                released: None,
            });
            need_update = true;
        }
    }

    if need_update {
        api.replace_status(
            &ap.name_any(),
            &PostParams::default(),
            serde_json::to_vec(&new_ap).map_err(Error::Serialization)?,
        )
        .await
        .map_err(Error::Kube)?;
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn reconcile_pod_pool(
    api: &Api<AddressPool>,
    ap: &AddressPool,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup(
    _api: &Api<AddressPool>,
    _ap: &AddressPool,
    _ctx: Arc<Context>,
) -> Result<Action, Error> {
    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let address_pools = Api::<AddressPool>::all(client.clone());
    if let Err(e) = address_pools.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start AddressPool reconciler");

    Controller::new(address_pools, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<AddressPool, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use http::Response;
    use hyper::{body::to_bytes, Body};
    use kube::{core::ObjectMeta, Api, ResourceExt};

    use crate::{
        context::Context,
        crd::{
            address_block::{AddressBlock, AddressBlockSpec},
            address_pool::{AddressPool, AddressPoolSpec, AddressPoolStatus},
        },
        error::Error,
        fixture::reconciler::{
            api_server_response_not_found, api_server_response_resource, assert_resource_request,
            timeout_after_1s, ApiServerVerifier,
        },
    };

    use super::reconcile;

    enum Scenario {
        Creation(AddressPool),
        UpdateNop(AddressPool),
        UpdateAddBlock(AddressPool),
    }

    impl ApiServerVerifier {
        fn address_pool_run(self, scenario: Scenario) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async move {
                match scenario {
                    Scenario::Creation(ap) => {
                        self.create_address_pool(&ap)
                            .await
                            .unwrap()
                            .update_address_pool_add_block(&ap)
                            .await
                    }
                    Scenario::UpdateNop(ap) => self.update_address_pool_nop(&ap).await,
                    Scenario::UpdateAddBlock(ap) => {
                        self.update_address_pool_nop(&ap)
                            .await
                            .unwrap()
                            .update_address_pool_add_block(&ap)
                            .await
                    }
                }
                .expect("scenario completed without error");
            })
        }

        async fn create_address_pool(mut self, ap: &AddressPool) -> Result<Self, Error> {
            let (request, send) = self.0.next_request().await.expect("service not called");
            let ab_non_exist = AddressBlock {
                metadata: ObjectMeta {
                    name: Some(ap.name_any()),
                    ..Default::default()
                },
                spec: AddressBlockSpec::default(),
                status: None,
            };
            assert_resource_request(
                &request,
                &ab_non_exist,
                None,
                false,
                None,
                http::Method::GET,
            );

            send.send_response(
                Response::builder()
                    .status(http::StatusCode::NOT_FOUND)
                    .body(Body::from(api_server_response_not_found(&ab_non_exist)))
                    .unwrap(),
            );

            let (request, send) = self.0.next_request().await.expect("service not called");
            assert_resource_request(
                &request,
                &ab_non_exist,
                None,
                false,
                None,
                http::Method::POST,
            );

            send.send_response(
                Response::builder()
                    .body(Body::from(api_server_response_resource(&ab_non_exist)))
                    .unwrap(),
            );

            // same procedure with update_add_block

            Ok(self)
        }

        async fn update_address_pool_nop(mut self, ap: &AddressPool) -> Result<Self, Error> {
            let (request, send) = self.0.next_request().await.expect("service not called");
            let ab = AddressBlock {
                metadata: ObjectMeta {
                    name: Some(ap.name_any()),
                    ..Default::default()
                },
                spec: AddressBlockSpec::default(),
                status: None,
            };
            assert_resource_request(&request, &ab, None, false, None, http::Method::GET);

            send.send_response(
                Response::builder()
                    .body(Body::from(api_server_response_resource(&ab)))
                    .unwrap(),
            );
            Ok(self)
        }

        async fn update_address_pool_add_block(mut self, ap: &AddressPool) -> Result<Self, Error> {
            let (request, send) = self.0.next_request().await.expect("service not called");
            let mut updated_ap = ap.clone();
            updated_ap.status = Some(AddressPoolStatus {
                requested: None,
                allocated: Some(vec![ap.name_any()]),
                released: None,
            });
            assert_resource_request(
                &request,
                &updated_ap,
                Some("status"),
                false,
                None,
                http::Method::PUT,
            );

            let json_req = to_bytes(request.into_body()).await.unwrap().to_vec();
            let json_expected = serde_json::to_vec(&updated_ap).unwrap();
            assert_json_diff::assert_json_eq!(json_req, json_expected);
            send.send_response(Response::builder().body(Body::from(json_expected)).unwrap());

            Ok(self)
        }
    }

    #[tokio::test]
    async fn address_pool_create_accepted() {
        let (testctx, fakeserver, _) = Context::test();
        let ap = AddressPool {
            metadata: ObjectMeta {
                name: Some("test-pool".to_string()),
                ..Default::default()
            },
            spec: AddressPoolSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: crate::crd::address_pool::AddressType::Service,
                alloc_type: None,
                block_size: 16,
                auto_assign: None,
            },
            status: None,
        };
        let mocksrv = fakeserver.address_pool_run(Scenario::Creation(ap.clone()));
        let api = Api::<AddressPool>::all(testctx.client.clone());
        reconcile(&api, &Arc::new(ap), testctx)
            .await
            .expect("reconciler");
        timeout_after_1s(mocksrv).await;
    }

    #[tokio::test]
    async fn address_pool_update_nop() {
        let (testctx, fakeserver, _) = Context::test();
        let ap = AddressPool {
            metadata: ObjectMeta {
                name: Some("test-pool".to_string()),
                ..Default::default()
            },
            spec: AddressPoolSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: crate::crd::address_pool::AddressType::Service,
                alloc_type: None,
                block_size: 16,
                auto_assign: None,
            },
            status: Some(AddressPoolStatus {
                requested: None,
                allocated: Some(vec!["test-pool".to_string()]),
                released: None,
            }),
        };
        let mocksrv = fakeserver.address_pool_run(Scenario::UpdateNop(ap.clone()));
        let api = Api::<AddressPool>::all(testctx.client.clone());
        reconcile(&api, &Arc::new(ap), testctx)
            .await
            .expect("reconciler");
        timeout_after_1s(mocksrv).await;
    }

    #[tokio::test]
    async fn address_pool_update_add_block() {
        let (testctx, fakeserver, _) = Context::test();
        let ap = AddressPool {
            metadata: ObjectMeta {
                name: Some("test-pool".to_string()),
                ..Default::default()
            },
            spec: AddressPoolSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: crate::crd::address_pool::AddressType::Service,
                alloc_type: None,
                block_size: 16,
                auto_assign: None,
            },
            status: None,
        };
        let mocksrv = fakeserver.address_pool_run(Scenario::UpdateAddBlock(ap.clone()));
        let api = Api::<AddressPool>::all(testctx.client.clone());
        reconcile(&api, &Arc::new(ap), testctx)
            .await
            .expect("reconciler");
        timeout_after_1s(mocksrv).await;
    }
}
