use std::{sync::Arc, time::Duration};

use futures::StreamExt;
use kube::{
    api::{ListParams, PostParams},
    runtime::{controller::Action, watcher::Config, Controller},
    Api, Client, ResourceExt,
};

use crate::{
    agent::{bgp::speaker, error::Error},
    context::{error_policy, Context, State},
    crd::{
        bgp_advertisement::{AdvertiseStatus, BGPAdvertisement, Protocol},
        bgp_peer::{BGPPeer, BGPPeerConditionStatus},
        node_bgp::NodeBGP,
    },
    util::get_namespace,
};

use super::node_bgp::{DEFAULT_SPEAKER_TIMEOUT, ENV_HOSTNAME};

#[tracing::instrument(skip_all)]
pub async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let bgp_advertisements = Api::<BGPAdvertisement>::all(client.clone());

    if let Err(e) = bgp_advertisements
        .list(&ListParams::default().limit(1))
        .await
    {
        tracing::error!("CRD is not queryable: {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start BGPAdvertisement watcher");

    let watch_config = Config::default();

    Controller::new(bgp_advertisements, watch_config.any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<BGPAdvertisement, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(ba: Arc<BGPAdvertisement>, ctx: Arc<Context>) -> Result<Action, Error> {
    let ns = get_namespace::<BGPAdvertisement>(&ba).map_err(Error::KubeLibrary)?;

    let bgp_advertisements = Api::<BGPAdvertisement>::namespaced(ctx.client.clone(), &ns);

    tracing::info!(
        name = ba.name_any(),
        namespace = ns,
        "Reconcile BGPAdvertisement"
    );

    reconcile(&bgp_advertisements, &ba, ctx).await
}

#[tracing::instrument(skip_all, fields(trace_id))]
async fn reconcile(
    api: &Api<BGPAdvertisement>,
    ba: &BGPAdvertisement,
    ctx: Arc<Context>,
) -> Result<Action, Error> {
    let node_bgps = Api::<NodeBGP>::all(ctx.client.clone());
    let node_name = std::env::var(ENV_HOSTNAME).map_err(Error::Var)?;

    let nb = node_bgps.get(&node_name).await.map_err(Error::Kube)?;
    let timeout = nb.spec.speaker.timeout.unwrap_or(DEFAULT_SPEAKER_TIMEOUT);
    let mut speaker_client =
        speaker::connect_bgp_with_retry(&nb.spec.speaker.path, Duration::from_secs(timeout))
            .await?;

    let family = sartd_proto::sart::AddressFamily {
        afi: match ba.spec.protocol {
            Protocol::IPv4 => sartd_proto::sart::address_family::Afi::Ip4.into(),
            Protocol::IPv6 => sartd_proto::sart::address_family::Afi::Ip6.into(),
        },
        safi: sartd_proto::sart::address_family::Safi::Unicast.into(),
    };

    let mut new_ba = ba.clone();
    let mut need_update = false;
    let mut need_requeue = false;

    if let Some(peers) = nb.spec.peers {
        let bgp_peers = Api::<BGPPeer>::all(ctx.client.clone());
        for p in peers.iter() {
            let bp = bgp_peers.get(&p.name).await.map_err(Error::Kube)?;
            if bp
                .status
                .as_ref()
                .and_then(|status| {
                    status.conditions.as_ref().and_then(|conds| {
                        conds.last().and_then(|cond| {
                            if cond.status != BGPPeerConditionStatus::Established {
                                None
                            } else {
                                Some(cond.status)
                            }
                        })
                    })
                })
                .is_none()
            {
                tracing::warn!(peer = bp.name_any(), "BGPPeer is not established");
                need_requeue = true;
                continue;
            }
            // peer is established
            if let Some(peers) = new_ba
                .status
                .as_mut()
                .and_then(|status| status.peers.as_mut())
            {
                if let Some(adv_status) = peers.get_mut(&p.name) {
                    match adv_status {
                        AdvertiseStatus::NotAdvertised => {
                            let res = speaker_client
                                .add_path(sartd_proto::sart::AddPathRequest {
                                    family: Some(family.clone()),
                                    prefixes: vec![ba.spec.cidr.clone()],
                                    attributes: Vec::new(), // TODO: implement attributes
                                })
                                .await
                                .map_err(Error::GotgPRC)?;
                            tracing::info!(name = ba.name_any(), namespace = ba.namespace(), status=?adv_status, response=?res,"Add path response");

                            *adv_status = AdvertiseStatus::Advertised;
                            need_update = true;
                        }
                        AdvertiseStatus::Advertised => {
                            let res = speaker_client
                                .add_path(sartd_proto::sart::AddPathRequest {
                                    family: Some(family.clone()),
                                    prefixes: vec![ba.spec.cidr.clone()],
                                    attributes: Vec::new(), // TODO: implement attributes
                                })
                                .await
                                .map_err(Error::GotgPRC)?;
                            tracing::info!(name = ba.name_any(), namespace = ba.namespace(), status=?adv_status ,response=?res,"Add path response");
                        }
                        AdvertiseStatus::Withdraw => {
                            let res = speaker_client
                                .delete_path(sartd_proto::sart::DeletePathRequest {
                                    family: Some(family.clone()),
                                    prefixes: vec![ba.spec.cidr.clone()],
                                })
                                .await
                                .map_err(Error::GotgPRC)?;
                            tracing::info!(name = ba.name_any(), namespace = ba.namespace(), status=?adv_status, response=?res,"Delete path response");

                            peers.remove(&p.name);
                            need_update = true;
                        }
                    }
                }
            }
        }

        if need_update {
            api.replace_status(
                &new_ba.name_any(),
                &PostParams::default(),
                serde_json::to_vec(&new_ba).map_err(Error::Serialization)?,
            )
            .await
            .map_err(Error::Kube)?;

            tracing::info!(
                name = ba.name_any(),
                namespace = ba.namespace(),
                "Update BGPAdvertisement"
            );
            return Ok(Action::requeue(Duration::from_secs(60)));
        }
    }
    if need_requeue {
        return Ok(Action::requeue(Duration::from_secs(10)));
    }
    Ok(Action::await_change())
}
