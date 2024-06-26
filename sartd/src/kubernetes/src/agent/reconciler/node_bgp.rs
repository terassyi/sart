use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

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
use tracing::{field, Span};

use crate::{
    agent::{
        bgp::speaker,
        context::{error_policy, Context, Ctx, State},
        error::Error,
        metrics::Metrics,
    },
    crd::{
        bgp_advertisement::{AdvertiseStatus, BGPAdvertisement},
        bgp_peer::{BGPPeer, BGPPeerCondition, BGPPeerConditionStatus},
        node_bgp::{
            NodeBGP, NodeBGPCondition, NodeBGPConditionReason, NodeBGPConditionStatus,
            NodeBGPStatus, NODE_BGP_FINALIZER,
        },
    },
    util::get_namespace,
};

pub const ENV_HOSTNAME: &str = "HOSTNAME";
pub const DEFAULT_SPEAKER_TIMEOUT: u64 = 10;

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(nb: Arc<NodeBGP>, ctx: Arc<Context>) -> Result<Action, Error> {
    let node_bgps = Api::<NodeBGP>::all(ctx.client.clone());

    ctx.metrics()
        .lock()
        .map_err(|_| Error::FailedToGetLock)?
        .reconciliation(nb.as_ref());

    finalizer(&node_bgps, NODE_BGP_FINALIZER, nb, |event| async {
        match event {
            Event::Apply(nb) => reconcile(&node_bgps, &nb, ctx).await,
            Event::Cleanup(nb) => cleanup(&node_bgps, &nb, ctx).await,
        }
    })
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(api: &Api<NodeBGP>, nb: &NodeBGP, ctx: Arc<Context>) -> Result<Action, Error> {
    let trace_id = sartd_trace::telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));

    // NodeBGP.spec.asn and routerId should be immutable

    let backoff = match nb.status.as_ref() {
        Some(status) => match status.conditions.as_ref() {
            Some(conds) => match conds.last() {
                Some(cond) => cond.status.eq(&NodeBGPConditionStatus::Available),
                None => false,
            },
            None => false,
        },
        None => false,
    };

    let timeout = nb.spec.speaker.timeout.unwrap_or(DEFAULT_SPEAKER_TIMEOUT);
    let mut speaker_client =
        match speaker::connect_bgp_with_retry(&nb.spec.speaker.path, Duration::from_secs(timeout))
            .await
        {
            Ok(client) => client,
            Err(e) => {
                if backoff {
                    let new_nb = backoff_node_bgp(nb);
                    api.replace_status(
                        &nb.name_any(),
                        &PostParams::default(),
                        serde_json::to_vec(&new_nb).map_err(Error::Serialization)?,
                    )
                    .await
                    .map_err(Error::Kube)?;
                    {
                        let metrics = ctx.metrics();
                        let metrics = metrics.lock().map_err(|_| Error::FailedToGetLock)?;
                        metrics.node_bgp_status_up(
                            &nb.name_any(),
                            &format!("{}", NodeBGPConditionStatus::Unavailable),
                        );
                        metrics.node_bgp_status_down(
                            &nb.name_any(),
                            &format!("{}", NodeBGPConditionStatus::Available),
                        );
                        metrics.node_bgp_backoff_count(&nb.name_any());
                    }
                }
                tracing::warn!(
                    name = nb.name_any(),
                    asn = nb.spec.asn,
                    router_id = nb.spec.router_id,
                    "backoff BGP advertisement"
                );
                backoff_advertisements(nb, &ctx.client.clone()).await?;
                return Err(e);
            }
        };

    let res = match speaker_client
        .get_bgp_info(sartd_proto::sart::GetBgpInfoRequest {})
        .await
    {
        Ok(res) => res,
        Err(e) => {
            if backoff {
                let new_nb = backoff_node_bgp(nb);
                api.replace_status(
                    &nb.name_any(),
                    &PostParams::default(),
                    serde_json::to_vec(&new_nb).map_err(Error::Serialization)?,
                )
                .await
                .map_err(Error::Kube)?;
                tracing::warn!(
                    name = nb.name_any(),
                    asn = nb.spec.asn,
                    router_id = nb.spec.router_id,
                    "backoff NodeBGP"
                );
                {
                    let metrics = ctx.metrics();
                    let metrics = metrics.lock().map_err(|_| Error::FailedToGetLock)?;
                    metrics.node_bgp_status_up(
                        &nb.name_any(),
                        &format!("{}", NodeBGPConditionStatus::Unavailable),
                    );
                    metrics.node_bgp_status_down(
                        &nb.name_any(),
                        &format!("{}", NodeBGPConditionStatus::Available),
                    );
                    metrics.node_bgp_backoff_count(&nb.name_any());
                }
                backoff_advertisements(nb, &ctx.client.clone()).await?;
                tracing::warn!(
                    name = nb.name_any(),
                    asn = nb.spec.asn,
                    router_id = nb.spec.router_id,
                    "backoff BGP advertisement"
                );
            }
            return Err(Error::GotgPRC(e));
        }
    };

    let info = res.get_ref().info.clone().ok_or(Error::FailedToGetData(
        "BGP information is not set".to_string(),
    ))?;
    if info.asn == 0 {
        tracing::info!(
            name = nb.name_any(),
            asn = nb.spec.asn,
            router_id = nb.spec.router_id,
            "configure local BGP settings"
        );
        speaker_client
            .set_as(sartd_proto::sart::SetAsRequest { asn: nb.spec.asn })
            .await?;
        speaker_client
            .set_router_id(sartd_proto::sart::SetRouterIdRequest {
                router_id: nb.spec.router_id.clone(),
            })
            .await?;

        if let Some(multi_path) = nb.spec.speaker.multipath {
            speaker_client
                .configure_multi_path(sartd_proto::sart::ConfigureMultiPathRequest {
                    enable: multi_path,
                })
                .await?;
        }

        let cond = NodeBGPCondition {
            status: NodeBGPConditionStatus::Available,
            reason: NodeBGPConditionReason::Configured,
        };
        let mut new_status = match &nb.status {
            Some(status) => {
                let mut new_status = status.clone();
                match new_status.conditions.as_mut() {
                    Some(conditions) => {
                        if let Some(last_cond) = conditions.last() {
                            if cond.ne(last_cond) {
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
                backoff: 0,
                cluster_bgp_refs: None,
                conditions: Some(vec![cond]),
            },
        };
        if backoff {
            backoff_advertisements(nb, &ctx.client.clone()).await?;
            new_status.backoff += 1;
        }
        tracing::info!(
            name = nb.name_any(),
            asn = nb.spec.asn,
            router_id = nb.spec.router_id,
            "update NodeBGP status"
        );
        // update status
        let mut new_nb = nb.clone();
        new_nb.status = Some(new_status);

        api.replace_status(
            &nb.name_any(),
            &PostParams::default(),
            serde_json::to_vec(&new_nb).map_err(Error::Serialization)?,
        )
        .await
        .map_err(Error::Kube)?;
        {
            let metrics = ctx.metrics();
            let metrics = metrics.lock().map_err(|_| Error::FailedToGetLock)?;
            metrics.node_bgp_status_down(
                &nb.name_any(),
                &format!("{}", NodeBGPConditionStatus::Unavailable),
            );
            metrics.node_bgp_status_up(
                &nb.name_any(),
                &format!("{}", NodeBGPConditionStatus::Available),
            );
        }

        // add peer's backoff count
        let bgp_peer_api = Api::<BGPPeer>::all(ctx.client.clone());
        if let Some(peers) = new_nb.spec.peers.as_ref() {
            for peer in peers.iter() {
                if let Some(mut bp) = bgp_peer_api
                    .get_opt(&peer.name)
                    .await
                    .map_err(Error::Kube)?
                {
                    if let Some(status) = bp.status.as_mut() {
                        status.backoff += 1;
                        match status.conditions.as_mut() {
                            Some(conds) => conds.push(BGPPeerCondition {
                                status: BGPPeerConditionStatus::Idle,
                                reason: "Changed by NodeBGP reconciler".to_string(),
                            }),
                            None => {
                                status.conditions = Some(vec![BGPPeerCondition {
                                    status: BGPPeerConditionStatus::Idle,
                                    reason: "Changed by NodeBGP reconciler".to_string(),
                                }])
                            }
                        }

                        tracing::info!(
                            name = nb.name_any(),
                            peer = bp.name_any(),
                            "increment peer's backoff count"
                        );
                        bgp_peer_api
                            .replace_status(
                                &bp.name_any(),
                                &PostParams::default(),
                                serde_json::to_vec(&bp).map_err(Error::Serialization)?,
                            )
                            .await
                            .map_err(Error::Kube)?;
                    }
                }
            }
        }

        // requeue immediately
        return Ok(Action::requeue(Duration::from_secs(1)));
    }

    let mut available = false;
    let cond = if info.asn == nb.spec.asn && info.router_id.eq(nb.spec.router_id.as_str()) {
        tracing::info!(
            name = nb.name_any(),
            asn = nb.spec.asn,
            router_id = nb.spec.router_id,
            "local BGP settings are already configured"
        );

        // patch status
        available = true;
        NodeBGPCondition {
            status: NodeBGPConditionStatus::Available,
            reason: NodeBGPConditionReason::Configured,
        }
    } else {
        tracing::warn!("local BGP speaker configuration and NodeBGP are mismatched");
        NodeBGPCondition {
            status: NodeBGPConditionStatus::Unavailable,
            reason: NodeBGPConditionReason::InvalidConfiguration,
        }
    };
    {
        let metrics = ctx.metrics();
        let metrics = metrics.lock().map_err(|_| Error::FailedToGetLock)?;
        match cond.status {
            NodeBGPConditionStatus::Available => {
                metrics.node_bgp_status_up(&nb.name_any(), &format!("{}", cond.status));
                metrics.node_bgp_status_down(
                    &nb.name_any(),
                    &format!("{}", NodeBGPConditionStatus::Unavailable),
                );
            }
            NodeBGPConditionStatus::Unavailable => {
                metrics.node_bgp_status_up(&nb.name_any(), &format!("{}", cond.status));
                metrics.node_bgp_status_down(
                    &nb.name_any(),
                    &format!("{}", NodeBGPConditionStatus::Available),
                );
            }
        }
    }

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
            backoff: 0,
            cluster_bgp_refs: None,
            conditions: Some(vec![cond]),
        },
    };
    if need_status_update {
        tracing::info!(
            name = nb.name_any(),
            asn = nb.spec.asn,
            router_id = nb.spec.router_id,
            "update NodeBGP status"
        );
        // update status
        new_nb.status = Some(new_status);
        api.replace_status(
            &nb.name_any(),
            &PostParams::default(),
            serde_json::to_vec(&new_nb).map_err(Error::Serialization)?,
        )
        .await
        .map_err(Error::Kube)?;

        // In this reconciliation, only updating status.
        return Ok(Action::requeue(Duration::from_secs(1)));
    }

    // create peers based on NodeBGP.spec.peers
    if available {
        let bgp_peers = Api::<BGPPeer>::all(ctx.client.clone());

        if let Some(peers) = new_nb.spec.peers.as_mut() {
            let mut errors: Vec<Error> = Vec::new();
            for p in peers.iter_mut() {
                let bp = p.from(nb);
                let bp_opt = match bgp_peers.get_opt(&bp.name_any()).await.map_err(Error::Kube) {
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
                            .map_err(Error::Kube)
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
                    tracing::error!(error=?e, "failed to reconcile BGPPeer associated with NodeBGP");
                }
                // returns ok but, this should retry to reconcile
                return Ok(Action::requeue(Duration::from_secs(10)));
            }
        }
    }

    Ok(Action::requeue(Duration::from_secs(60)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(_api: &Api<NodeBGP>, nb: &NodeBGP, _ctx: Arc<Context>) -> Result<Action, Error> {
    let trace_id = sartd_trace::telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));

    let timeout = nb.spec.speaker.timeout.unwrap_or(DEFAULT_SPEAKER_TIMEOUT);
    let mut speaker_client =
        speaker::connect_bgp_with_retry(&nb.spec.speaker.path, Duration::from_secs(timeout))
            .await?;

    match speaker_client
        .list_neighbor(sartd_proto::sart::ListNeighborRequest {})
        .await
    {
        Ok(peers) => {
            if !peers.get_ref().peers.is_empty() {
                return Err(Error::PeerExists);
            }
        }
        Err(status) => {
            if status.code() != tonic::Code::NotFound {
                return Err(Error::GotgPRC(status));
            }
        }
    }

    if let Err(status) = speaker_client
        .clear_bgp_info(sartd_proto::sart::ClearBgpInfoRequest {})
        .await
    {
        return Err(Error::GotgPRC(status));
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all)]
pub async fn run(state: State, interval: u64, metrics: Arc<Mutex<Metrics>>) {
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
            error_policy::<NodeBGP, Error, Context>,
            state.to_context(client, interval, metrics),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

fn backoff_node_bgp(nb: &NodeBGP) -> NodeBGP {
    let mut new_nb = nb.clone();

    let backoff_cond = NodeBGPCondition {
        status: NodeBGPConditionStatus::Unavailable,
        reason: NodeBGPConditionReason::InvalidConfiguration,
    };

    match new_nb.status.as_mut() {
        Some(status) => {
            match status.conditions.as_mut() {
                Some(conds) => conds.push(backoff_cond),
                None => status.conditions = Some(vec![backoff_cond]),
            }
            status.backoff += 1;
        }
        None => {
            new_nb.status = Some(NodeBGPStatus {
                backoff: 1,
                cluster_bgp_refs: None,
                conditions: Some(vec![NodeBGPCondition {
                    status: NodeBGPConditionStatus::Unavailable,
                    reason: NodeBGPConditionReason::InvalidConfiguration,
                }]),
            })
        }
    }

    new_nb
}

async fn backoff_advertisements(nb: &NodeBGP, client: &Client) -> Result<(), Error> {
    let bgp_advertisement_api = Api::<BGPAdvertisement>::all(client.clone());
    let nb_peers = match nb.spec.peers.as_ref() {
        Some(peers) => peers
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<String>>(),
        None => return Ok(()),
    };

    let ba_list = bgp_advertisement_api
        .list(&ListParams::default())
        .await
        .map_err(Error::Kube)?;

    for ba in ba_list.iter() {
        let mut new_ba = ba.clone();
        if let Some(status) = new_ba.status.as_mut() {
            if let Some(peers) = status.peers.as_mut() {
                let mut need_update = false;
                for peer in nb_peers.iter() {
                    if let Some(adv_status) = peers.get_mut(peer) {
                        if AdvertiseStatus::NotAdvertised.ne(adv_status) {
                            *adv_status = AdvertiseStatus::NotAdvertised;
                            need_update = true;
                        }
                    }
                }
                if need_update {
                    let ns = get_namespace(ba).map_err(Error::KubeLibrary)?;
                    let ns_bgp_advertisement_api =
                        Api::<BGPAdvertisement>::namespaced(client.clone(), &ns);
                    ns_bgp_advertisement_api
                        .replace_status(
                            &new_ba.name_any(),
                            &PostParams::default(),
                            serde_json::to_vec(&new_ba).map_err(Error::Serialization)?,
                        )
                        .await
                        .map_err(Error::Kube)?;
                }
            }
        }
    }
    Ok(())
}
