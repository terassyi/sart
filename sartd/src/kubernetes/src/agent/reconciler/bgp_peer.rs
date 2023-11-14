use std::{sync::Arc, time::Duration};

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

use crate::{
    agent::{bgp::speaker, error::Error},
    context::{error_policy, Context, State},
    crd::{
        bgp_peer::{
            BGPPeer, BGPPeerCondition, BGPPeerConditionStatus, BGPPeerSlim, BGPPeerStatus,
            BGP_PEER_FINALIZER, BGP_PEER_NODE_LABEL,
        },
        node_bgp::NodeBGP,
    },
};

use super::node_bgp::{DEFAULT_SPEAKER_TIMEOUT, ENV_HOSTNAME};

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconciler(bp: Arc<BGPPeer>, ctx: Arc<Context>) -> Result<Action, Error> {
    let bgp_peers = Api::<BGPPeer>::all(ctx.client.clone());

    finalizer(&bgp_peers, BGP_PEER_FINALIZER, bp, |event| async {
        match event {
            Event::Apply(bp) => reconcile(&bgp_peers, &bp, ctx).await,
            Event::Cleanup(bp) => cleanup(&bgp_peers, &bp, ctx).await,
        }
    })
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(api: &Api<BGPPeer>, bp: &BGPPeer, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(name = bp.name_any(), "Reconcile BGPPeer");

    let timeout = match bp.spec.speaker.timeout {
        Some(t) => t,
        None => DEFAULT_SPEAKER_TIMEOUT,
    };
    let mut speaker_client =
        speaker::connect_bgp_with_retry(&bp.spec.speaker.path, Duration::from_secs(timeout))
            .await?;
    let res = speaker_client
        .get_bgp_info(sartd_proto::sart::GetBgpInfoRequest {})
        .await?;
    let info = res.get_ref().info.clone().ok_or(Error::FailedToGetData(
        "BGP information is not set".to_string(),
    ))?;
    if info.asn == 0 {
        tracing::warn!(
            node_bgp = bp.spec.node_bgp_ref,
            "Local BGP speaker is not configured"
        );
        return Ok(Action::requeue(Duration::from_secs(1)));
    }

    // add peer
    let mut new_bp = bp.clone();

    // create or update peer and its status
    match speaker_client
        .get_neighbor(sartd_proto::sart::GetNeighborRequest {
            addr: bp.spec.addr.clone(),
        })
        .await
    {
        Ok(peer) => {
            let mut need_status_update = false;
            match &peer.get_ref().peer {
                Some(peer) => {
                    tracing::info!(
                        asn = bp.spec.asn,
                        addr = bp.spec.addr,
                        "Peer already exists"
                    );
                    // update status
                    match new_bp.status.as_mut() {
                        Some(status) => match status.conditions.as_mut() {
                            Some(conditions) => {
                                if let Some(cond) = conditions.last() {
                                    if cond.status as i32 != peer.state {
                                        let new_state =
                                            BGPPeerConditionStatus::try_from(peer.state)
                                                .map_err(Error::CRD)?;
                                        tracing::info!(
                                            name = bp.name_any(),
                                            asn = bp.spec.asn,
                                            addr = bp.spec.addr,
                                            old_state = ?cond.status,
                                            new_state = ?new_state,
                                            "Peer state is changed"
                                        );
                                        conditions.push(BGPPeerCondition {
                                            status: BGPPeerConditionStatus::try_from(peer.state)
                                                .map_err(Error::CRD)?,
                                            reason: "".to_string(),
                                        });
                                        need_status_update = true;
                                    }
                                }
                                // when BGPPeer's status and actual peer state got from the speaker is same, do nothing
                            }
                            None => {
                                let state = BGPPeerConditionStatus::try_from(peer.state)
                                    .map_err(Error::CRD)?;
                                tracing::info!(
                                    name = bp.name_any(),
                                    asn = bp.spec.asn,
                                    addr = bp.spec.addr,
                                    state = ?state,
                                    "Peer state is initialized"
                                );
                                status.conditions = Some(vec![BGPPeerCondition {
                                    status: state,
                                    reason: "".to_string(),
                                }]);
                                need_status_update = true;
                            }
                        },
                        None => {
                            let state =
                                BGPPeerConditionStatus::try_from(peer.state).map_err(Error::CRD)?;
                            tracing::info!(
                                name = bp.name_any(),
                                asn = bp.spec.asn,
                                addr = bp.spec.addr,
                                state = ?state,
                                "Peer state is initialized"
                            );
                            new_bp.status = Some(BGPPeerStatus {
                                conditions: Some(vec![BGPPeerCondition {
                                    status: state,
                                    reason: "".to_string(),
                                }]),
                            });
                            need_status_update = true;
                        }
                    }
                }
                None => {
                    return Err(Error::FailedToGetData(
                        "failed to get peer information from speaker".to_string(),
                    ));
                }
            }
            if need_status_update {
                // update peer state
                tracing::info!(
                    name = bp.name_any(),
                    asn = bp.spec.asn,
                    addr = bp.spec.addr,
                    "Update BGPPeer status"
                );
                api.replace_status(
                    &bp.name_any(),
                    &PostParams::default(),
                    serde_json::to_vec(&new_bp).map_err(Error::Serialization)?,
                )
                .await
                .map_err(Error::Kube)?;
            }
        }
        Err(status) => {
            if status.code() != tonic::Code::NotFound {
                return Err(Error::GotgPRC(status));
            }
            // When peer doesn't exist
            tracing::info!(
                asn = bp.spec.asn,
                addr = bp.spec.addr,
                "Peer doesn't exist yet"
            );

            speaker_client
                .add_peer(sartd_proto::sart::AddPeerRequest {
                    peer: Some(sartd_proto::sart::Peer {
                        name: bp.name_any(),
                        asn: bp.spec.asn,
                        address: bp.spec.addr.clone(),
                        router_id: bp.spec.addr.clone(),
                        families: vec![sartd_proto::sart::AddressFamily {
                            afi: sartd_proto::sart::address_family::Afi::Ip4 as i32,
                            safi: sartd_proto::sart::address_family::Safi::Unicast as i32,
                        }],
                        hold_time: bp.spec.hold_time.unwrap_or(0),
                        keepalive_time: bp.spec.keepalive_time.unwrap_or(0),
                        uptime: None,
                        send_counter: None,
                        recv_counter: None,
                        state: 0,
                        passive_open: false,
                    }),
                })
                .await
                .map_err(Error::GotgPRC)?;

            tracing::info!(asn = bp.spec.asn, addr = bp.spec.addr, "Create Peer");

            // Update NodeBGP
            let node_bgps = Api::<NodeBGP>::all(ctx.client.clone());
            let mut nb = node_bgps
                .get(&bp.spec.node_bgp_ref)
                .await
                .map_err(Error::Kube)?;

            match nb.spec.peers.as_mut() {
                Some(peers) => {
                    if !peers.iter().any(|p| p.name == bp.name_any()) {
                        peers.push(BGPPeerSlim::into(bp));
                    }
                }
                None => nb.spec.peers = Some(vec![BGPPeerSlim::into(bp)]),
            }

            node_bgps
                .replace(&nb.name_any(), &PostParams::default(), &nb)
                .await
                .map_err(Error::Kube)?;

            // Reconcile again after 5 second for synchronizing BGP peer state
            return Ok(Action::requeue(Duration::from_secs(5)));
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn cleanup(_api: &Api<BGPPeer>, bp: &BGPPeer, _ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(name = bp.name_any(), "Cleanup BGPPeer");

    let timeout = match bp.spec.speaker.timeout {
        Some(t) => t,
        None => DEFAULT_SPEAKER_TIMEOUT,
    };
    let mut speaker_client =
        speaker::connect_bgp_with_retry(&bp.spec.speaker.path, Duration::from_secs(timeout))
            .await?;

    match speaker_client
        .get_neighbor(sartd_proto::sart::GetNeighborRequest {
            addr: bp.spec.addr.clone(),
        })
        .await
    {
        Ok(_peer) => {
            tracing::info!(name = bp.name_any(), addr = bp.spec.addr, "Delete peer");
            speaker_client
                .delete_peer(sartd_proto::sart::DeletePeerRequest {
                    addr: bp.spec.addr.clone(),
                })
                .await
                .map_err(Error::GotgPRC)?;
        }
        Err(status) => {
            if status.code() != tonic::Code::NotFound {
                return Err(Error::GotgPRC(status));
            }
        }
    }

    Ok(Action::await_change())
}

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let bgp_peers = Api::<BGPPeer>::all(client.clone());
    if let Err(e) = bgp_peers.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start BGPPeer reconciler");

    let node_name = std::env::var(ENV_HOSTNAME).expect("HOSTNAME environment value is not set");
    let label_selector = format!("{}={}", BGP_PEER_NODE_LABEL, node_name);

    let watch_config = Config::default().labels(&label_selector);
    Controller::new(bgp_peers, watch_config.any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<BGPPeer, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
