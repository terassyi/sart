use k8s_openapi::api::discovery::v1::EndpointSlice;
use kube::{
    api::{ListParams, Patch, PatchParams, PostParams},
    Api, Client, ResourceExt,
};
use sartd_proto::sart::{
    bgp_exporter_api_server::{BgpExporterApi, BgpExporterApiServer},
    ExportPeerRequest, ExportPeerStateRequest,
};
use tonic::{transport::Server, Request, Response, Status};

use crate::{
    agent::error::Error,
    controller::reconciler::endpointslice_watcher::ENDPOINTSLICE_TRIGGER,
    crd::{
        bgp_advertisement::{AdvertiseStatus, BGPAdvertisement},
        bgp_peer::{BGPPeer, BGPPeerCondition, BGPPeerConditionStatus, BGPPeerStatus},
    },
    util::get_namespace,
};

pub struct BGPPeerStateWatcher {
    pub client: Client,
    pub api: Api<BGPPeer>,
}

impl BGPPeerStateWatcher {
    pub fn new(client: Client) -> Self {
        let api = Api::<BGPPeer>::all(client.clone());
        Self { client, api }
    }
}

#[tonic::async_trait]
impl BgpExporterApi for BGPPeerStateWatcher {
    async fn export_peer(&self, _req: Request<ExportPeerRequest>) -> Result<Response<()>, Status> {
        Ok(Response::new(()))
    }

    #[tracing::instrument(skip_all)]
    async fn export_peer_state(
        &self,
        req: Request<ExportPeerStateRequest>,
    ) -> Result<Response<()>, Status> {
        let info = req.get_ref();
        let state = match BGPPeerConditionStatus::try_from(info.state) {
            Ok(s) => s,
            Err(e) => return Err(Status::aborted(e.to_string())),
        };
        tracing::info!(asn=info.asn, addr= info.addr,state =?state,"Peer state is changed");

        let bp = match self.api.get_status(&info.name).await {
            Ok(b) => b,
            Err(e) => return Err(Status::aborted(e.to_string())),
        };

        let mut new_bp = bp.clone();
        let mut need_update = true;

        let mut established = false;
        let mut reset = false;

        match new_bp.status.as_mut() {
            Some(status) => match status.conditions.as_mut() {
                Some(conditions) => {
                    if let Some(cond) = conditions.last() {
                        let old_status = cond.status;
                        if state.ne(&old_status) {
                            // sync actual state
                            conditions.push(BGPPeerCondition {
                                status: state,
                                reason: "Synchronized by watcher".to_string(),
                            });
                            if state.eq(&BGPPeerConditionStatus::Established) {
                                established = true;
                            }
                            if old_status.eq(&BGPPeerConditionStatus::Established)
                                && state.ne(&BGPPeerConditionStatus::Established)
                            {
                                reset = true;
                            }
                        } else {
                            need_update = false;
                        }
                    } else {
                        conditions.push(BGPPeerCondition {
                            status: state,
                            reason: "Synchronized by watcher".to_string(),
                        });
                        if state.eq(&BGPPeerConditionStatus::Established) {
                            established = true;
                        }
                    }
                }
                None => {
                    status.conditions = Some(vec![BGPPeerCondition {
                        status: state,
                        reason: "Synchronized by watcher".to_string(),
                    }]);
                    if state.eq(&BGPPeerConditionStatus::Established) {
                        established = true;
                    }
                }
            },
            None => {
                new_bp.status = Some(BGPPeerStatus {
                    backoff: 0,
                    conditions: Some(vec![BGPPeerCondition {
                        status: state,
                        reason: "Synchronized by watcher".to_string(),
                    }]),
                });
                if state.eq(&BGPPeerConditionStatus::Established) {
                    established = true;
                }
            }
        }

        if need_update {
            let data = match serde_json::to_vec(&new_bp) {
                Ok(d) => d,
                Err(e) => return Err(Status::internal(e.to_string())),
            };
            tracing::info!(
                name = bp.name_any(),
                asn = bp.spec.asn,
                addr = bp.spec.addr,
                state =? state,
                "Peer state is changed by watcher"
            );
            if let Err(e) = self
                .api
                .replace_status(&new_bp.name_any(), &PostParams::default(), data)
                .await
            {
                return Err(Status::internal(e.to_string()));
            }
        }

        let client = match Client::try_default().await.map_err(Error::Kube) {
            Ok(client) => client,
            Err(e) => return Err(Status::internal(e.to_string())),
        };

        if established {
            // for newly established peer
            tracing::info!(
                name = bp.name_any(),
                asn = bp.spec.asn,
                addr = bp.spec.addr,
                "Reflect the newly established peer to existing BGPAdvertisements"
            );
            let eps_api = Api::<EndpointSlice>::all(client.clone());
            let mut eps_list = match eps_api
                .list(&ListParams::default())
                .await
                .map_err(Error::Kube)
            {
                Ok(l) => l,
                Err(e) => return Err(Status::internal(e.to_string())),
            };
            for eps in eps_list.iter_mut() {
                eps.labels_mut()
                    .insert(ENDPOINTSLICE_TRIGGER.to_string(), bp.name_any());
                let ns = match get_namespace::<EndpointSlice>(eps).map_err(Error::KubeLibrary) {
                    Ok(n) => n,
                    Err(e) => return Err(Status::internal(e.to_string())),
                };
                let ns_eps_api = Api::<EndpointSlice>::namespaced(client.clone(), &ns);
                let eps_name = eps.name_any();
                let ssapply = PatchParams::apply("agent-peerwatcher");
                let patch = Patch::Merge(eps);
                if let Err(e) = ns_eps_api
                    .patch(&eps_name, &ssapply, &patch)
                    .await
                    .map_err(Error::Kube)
                {
                    return Err(Status::internal(e.to_string()));
                }
            }
        }

        if reset {
            // A peer is no longer established.
            tracing::info!(
                name = bp.name_any(),
                asn = bp.spec.asn,
                addr = bp.spec.addr,
                "Reset BGPAdvertisements"
            );
            let ba_api = Api::<BGPAdvertisement>::all(client.clone());
            let mut ba_list = match ba_api
                .list(&ListParams::default())
                .await
                .map_err(Error::Kube)
            {
                Ok(l) => l,
                Err(e) => return Err(Status::internal(e.to_string())),
            };
            for ba in ba_list.iter_mut() {
                if let Some(status) = ba.status.as_mut() {
                    if let Some(peers) = status.peers.as_mut() {
                        if let Some(adv_status) = peers.get_mut(&bp.name_any()) {
                            if AdvertiseStatus::Advertised.eq(adv_status) {
                                *adv_status = AdvertiseStatus::NotAdvertised;
                                let ns = match get_namespace::<BGPAdvertisement>(ba)
                                    .map_err(Error::KubeLibrary)
                                {
                                    Ok(n) => n,
                                    Err(e) => return Err(Status::internal(e.to_string())),
                                };
                                let data =
                                    match serde_json::to_vec(&ba).map_err(Error::Serialization) {
                                        Ok(d) => d,
                                        Err(e) => return Err(Status::internal(e.to_string())),
                                    };
                                let ns_ba_api =
                                    Api::<BGPAdvertisement>::namespaced(client.clone(), &ns);
                                if let Err(e) = ns_ba_api
                                    .replace_status(&ba.name_any(), &PostParams::default(), data)
                                    .await
                                    .map_err(Error::Kube)
                                {
                                    return Err(Status::internal(e.to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(Response::new(()))
    }
}

#[tracing::instrument()]
pub async fn run(endpoint: &str) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube config");

    let sock_addr = endpoint.parse().unwrap();

    tracing::info!("Peer state watcher is started at {}", endpoint);

    Server::builder()
        .add_service(BgpExporterApiServer::new(BGPPeerStateWatcher::new(client)))
        .serve(sock_addr)
        .await
        .unwrap();
}
