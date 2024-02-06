use std::sync::Arc;

use futures::StreamExt;
use kube::{api::{ListParams, PostParams}, runtime::{controller::Action, finalizer::{finalizer, Event}, watcher::Config, Controller}, Api, Client, ResourceExt};


use crate::{context::{error_policy, Context, Ctx, State}, controller::error::Error, crd::{address_pool::{AddressPool, AddressPoolStatus, AddressType}, block_request::{BlockRequest, BLOCK_REQUEST_FINALIZER}}};


#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(br: Arc<BlockRequest>, ctx: Arc<Context>) -> Result<Action, Error> {

	let block_request_api = Api::<BlockRequest>::all(ctx.client().clone());

	finalizer(&block_request_api, BLOCK_REQUEST_FINALIZER, br, |event| async {
		match event {
			Event::Apply(br) => reconcile(&block_request_api, &br, ctx.clone()).await,
			Event::Cleanup(br) => cleanup(&block_request_api, &br, ctx.clone()).await,
		}
	}).await.map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all)]
async fn reconcile(_api: &Api<BlockRequest>, br: &BlockRequest, ctx: Arc<Context>) -> Result<Action, Error> {
	tracing::info!(name = br.name_any(), "reconcile BlockRequest");

	let address_pool_api = Api::<AddressPool>::all(ctx.client.clone());

	let mut pool = address_pool_api.get(&br.spec.pool).await.map_err(Error::Kube)?;

	if pool.spec.r#type.ne(&AddressType::Pod) {
		tracing::error!(name=br.name_any(), "The requesting pool is not for Pods.");
		return Err(Error::InvalidAddressType);
	}

	match pool.status.as_mut() {
		Some(status) => {
			match status.requested.as_mut() {
				Some(requested) => {
					if requested.iter().any(|r| r.eq(&br.name_any())) {
						tracing::warn!(name=br.name_any(), "Same BlockRequest already exists");
						return Err(Error::BlockRequestAlreadyExists);
					}
					requested.push(br.name_any());
				},
				None => {
					status.requested = Some(vec![br.name_any()]);
				}
			}
		},
		None => {
			pool.status = Some(AddressPoolStatus{
				requested: Some(vec![br.name_any()]),
				allocated: None,
				released: None,
			});
		}
	}

	address_pool_api.replace_status(&pool.name_any(), &PostParams::default(), serde_json::to_vec(&pool).map_err(Error::Serialization)?).await.map_err(Error::Kube)?;

	Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup(_api: &Api<BlockRequest>, br: &BlockRequest, ctx: Arc<Context>) -> Result<Action, Error> {
	tracing::info!(name = br.name_any(), "clean up BlockRequest");

	let address_pool_api = Api::<AddressPool>::all(ctx.client.clone());

	let pool = address_pool_api.get(&br.spec.pool).await.map_err(Error::Kube)?;

	if let Some(status) = pool.status.as_ref() {
		if let Some(requested) = status.requested.as_ref() {
			if requested.iter().any(|r| r.eq(&br.name_any())) {
				tracing::warn!(name=br.name_any(), "BlockRequest isn't performed yet.");
				return Err(Error::BlockRequestNotPerformed);
			}
		}
	}

	Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

	let block_request_api = Api::<BlockRequest>::all(client.clone());
	if let Err(e) = block_request_api.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
	}

	tracing::info!("Start BlockRequest reconciler");

    Controller::new(block_request_api, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<BlockRequest, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
