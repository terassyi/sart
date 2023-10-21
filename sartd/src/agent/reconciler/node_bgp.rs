use std::{sync::Arc, time::Duration};

use futures::StreamExt;

use kube::{
    api::{ListParams, PostParams},
    runtime::{
        controller::{Action, Controller},
        finalizer::{finalizer, Event},
        watcher::Config,
    },
    Api, Client, ResourceExt,
};

use crate::{
    agent::error::Error,
    kubernetes::{
        context::{error_policy, Context, State},
        crd::{
            bgp_peer::BGPPeer,
            node_bgp::{
                NodeBGP, NodeBGPCondition, NodeBGPConditionReason, NodeBGPConditionStatus,
                NodeBGPStatus, NODE_BGP_FINALIZER,
            },
        },
    },
};

use crate::agent::bgp::speaker;

pub(crate) const ENV_HOSTNAME: &str = "HOSTNAME";
pub(crate) const DEFAULT_SPEAKER_TIMEOUT: u64 = 10;

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(nb: Arc<NodeBGP>, ctx: Arc<Context>) -> Result<Action, Error> {
    let node_bgps = Api::<NodeBGP>::all(ctx.client.clone());

    finalizer(&node_bgps, NODE_BGP_FINALIZER, nb, |event| async {
        match event {
            Event::Apply(nb) => reconcile(&node_bgps, &nb, ctx).await,
            Event::Cleanup(nb) => cleanup(&node_bgps, &nb, ctx).await,
        }
    })
    .await
    .map_err(|e| Error::FinalizerError(Box::new(e)))
}

#[tracing::instrument(skip_all)]
async fn reconcile(api: &Api<NodeBGP>, nb: &NodeBGP, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(name = nb.name_any(), "Reconcile NodeBGP");

    // NodeBGP.spec.asn and routerId should be immutable

    let timeout = match nb.spec.speaker.timeout {
        Some(t) => t,
        None => DEFAULT_SPEAKER_TIMEOUT,
    };
    let mut speaker_client =
        speaker::connect_bgp_with_retry(&nb.spec.speaker.path, Duration::from_secs(timeout))
            .await?;

    let res = speaker_client
        .get_bgp_info(crate::proto::sart::GetBgpInfoRequest {})
        .await?;
    let info = res.get_ref().info.clone().ok_or(Error::FailedToGetData(
        "BGP information is not set".to_string(),
    ))?;
    if info.asn == 0 {
        tracing::info!(
            name = nb.name_any(),
            asn = nb.spec.asn,
            router_id = nb.spec.router_id,
            "Configure local BGP settings"
        );
        speaker_client
            .set_as(crate::proto::sart::SetAsRequest { asn: nb.spec.asn })
            .await?;
        speaker_client
            .set_router_id(crate::proto::sart::SetRouterIdRequest {
                router_id: nb.spec.router_id.clone(),
            })
            .await?;

        let cond = NodeBGPCondition::default();
        let new_status = match &nb.status {
            Some(status) => {
                let mut new_status = status.clone();
                match new_status.conditions.as_mut() {
                    Some(conditions) => conditions.push(cond),
                    None => new_status.conditions = Some(vec![cond]),
                }
                new_status
            }
            None => NodeBGPStatus {
                conditions: Some(vec![cond]),
            },
        };
        tracing::info!(
            name = nb.name_any(),
            asn = nb.spec.asn,
            router_id = nb.spec.router_id,
            "Update NodeBGP status"
        );
        // update status
        let mut new_nb = nb.clone();
        new_nb.status = Some(new_status);

        api.replace_status(
            &nb.name_any(),
            &PostParams::default(),
            serde_json::to_vec(&new_nb).map_err(Error::SerializationError)?,
        )
        .await
        .map_err(Error::KubeError)?;

        // requeue immediately
        return Ok(Action::requeue(Duration::from_secs(1)));
    }

    let mut available = false;
    let cond = if info.asn == nb.spec.asn && info.router_id.eq(nb.spec.router_id.as_str()) {
        tracing::info!(
            name = nb.name_any(),
            asn = nb.spec.asn,
            router_id = nb.spec.router_id,
            "Local BGP settings are already configured"
        );

        // patch status
        available = true;
        NodeBGPCondition {
            status: NodeBGPConditionStatus::Available,
            reason: NodeBGPConditionReason::Configured,
        }
    } else {
        tracing::warn!("Local BGP speaker configuration and NodeBGP are mismatched");
        NodeBGPCondition {
            status: NodeBGPConditionStatus::Unavailable,
            reason: NodeBGPConditionReason::InvalidConfiguration,
        }
    };

    let mut need_status_update = true;
    let mut new_nb = nb.clone();
    let new_status = match &nb.status {
        Some(status) => {
            let mut new_status = status.clone();
            match new_status.conditions.as_mut() {
                Some(conditions) => {
                    if let Some(c) = conditions.last() {
                        if c.status.eq(&cond.status) && c.reason.eq(&cond.reason) {
                            need_status_update = false;
                        } else {
                            conditions.push(cond);
                        }
                    } else {
                        conditions.push(cond);
                    }
                }
                None => new_status.conditions = Some(vec![cond]),
            }
            new_status
        }
        None => NodeBGPStatus {
            conditions: Some(vec![cond]),
        },
    };
    if need_status_update {
        tracing::info!(
            name = nb.name_any(),
            asn = nb.spec.asn,
            router_id = nb.spec.router_id,
            "Update NodeBGP status"
        );
        // update status
        new_nb.status = Some(new_status);
        api.replace_status(
            &nb.name_any(),
            &PostParams::default(),
            serde_json::to_vec(&new_nb).map_err(Error::SerializationError)?,
        )
        .await
        .map_err(Error::KubeError)?;
    }

    // create peers based on NodeBGP.spec.peers
    if available {
        let bgp_peers = Api::<BGPPeer>::all(ctx.client.clone());
        if let Some(peers) = new_nb.spec.peers.as_mut() {
            let mut errors: Vec<Error> = Vec::new();
            for p in peers.iter_mut() {
                let bp = p.from(nb);
                let bp_opt = match bgp_peers
                    .get_opt(&bp.name_any())
                    .await
                    .map_err(Error::KubeError)
                {
                    Ok(bp_opt) => bp_opt,
                    Err(e) => {
                        errors.push(e);
                        continue;
                    }
                };
                match bp_opt {
                    Some(_bpp) => {
                        // we should take care of capabilities
                    }
                    None => {
                        match bgp_peers
                            .create(&PostParams::default(), &bp)
                            .await
                            .map_err(Error::KubeError)
                        {
                            Ok(_) => {}
                            Err(e) => {
                                errors.push(e);
                                continue;
                            }
                        }
                    }
                };
            }
            if !errors.is_empty() {
                for e in errors.iter() {
                    tracing::error!(error=?e, "Failed to reconcile BGPPeer associated with NodeBGP");
                }
                // returns ok but, this should retry to reconcile
                return Ok(Action::requeue(Duration::from_secs(10)));
            }
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
async fn cleanup(api: &Api<NodeBGP>, nb: &NodeBGP, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(name = nb.name_any(), "Clean up NodeBGP");
    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
pub(crate) async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let node_bgps = Api::<NodeBGP>::all(client.clone());
    if let Err(e) = node_bgps.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    // NodeBGP resource must be one per node, once some ClusterBGP resource are created.
    // And it must be named as node's name
    let node_name = std::env::var(ENV_HOSTNAME).expect("HOSTNAME environment value is not set");

    tracing::info!(node = node_name, "Start NodeBGP reconciler");
    let field_selector = format!("metadata.name={}", node_name);

    let watch_config = Config::default().fields(&field_selector);
    Controller::new(node_bgps, watch_config.any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<NodeBGP, Error>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
