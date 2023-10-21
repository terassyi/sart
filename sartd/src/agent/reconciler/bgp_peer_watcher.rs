use std::{f32::consts::E, sync::Arc};

use kube::{api::PostParams, Api, Client, ResourceExt};
use tonic::{transport::Server, Request, Response, Status};
use tonic_reflection::server::Builder;

use crate::{
    kubernetes::crd::bgp_peer::{BGPPeer, BGPPeerCondition, BGPPeerConditionStatus, BGPPeerStatus},
    proto::sart::{
        bgp_exporter_api_server::{BgpExporterApi, BgpExporterApiServer},
        ExportPeerRequest, ExportPeerStateRequest,
    },
};

pub(crate) struct BGPPeerStateWatcher {
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

        match new_bp.status.as_mut() {
            Some(status) => match status.conditions.as_mut() {
                Some(conditions) => {
                    if let Some(cond) = conditions.last() {
                        if state.ne(&cond.status) {
                            // sync actual state
                            conditions.push(BGPPeerCondition {
                                status: state,
                                reason: "Synchronized by watcher".to_string(),
                            });
                        } else {
                            need_update = false;
                        }
                    } else {
                        conditions.push(BGPPeerCondition {
                            status: state,
                            reason: "Synchronized by watcher".to_string(),
                        });
                    }
                }
                None => {
                    status.conditions = Some(vec![BGPPeerCondition {
                        status: state,
                        reason: "Synchronized by watcher".to_string(),
                    }]);
                }
            },
            None => {
                new_bp.status = Some(BGPPeerStatus {
                    conditions: Some(vec![BGPPeerCondition {
                        status: state,
                        reason: "Synchronized by watcher".to_string(),
                    }]),
                });
            }
        }

        if need_update {
            let data = match serde_json::to_vec(&new_bp) {
                Ok(d) => d,
                Err(e) => return Err(Status::internal(e.to_string())),
            };
            if let Err(e) = self
                .api
                .replace_status(&new_bp.name_any(), &PostParams::default(), data)
                .await
            {
                return Err(Status::internal(e.to_string()));
            }
        }

        Ok(Response::new(()))
    }
}

#[tracing::instrument()]
pub(crate) async fn run(endpoint: &str) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube config");
    let reflection_server = Builder::configure()
        .register_encoded_file_descriptor_set(api::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    let sock_addr = endpoint.parse().unwrap();

    tracing::info!("Peer state watcher is started at {}", endpoint);

    Server::builder()
        .add_service(BgpExporterApiServer::new(BGPPeerStateWatcher::new(client)))
        .add_service(reflection_server)
        .serve(sock_addr)
        .await
        .unwrap();
}

pub mod api {
    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("sartd");
}
