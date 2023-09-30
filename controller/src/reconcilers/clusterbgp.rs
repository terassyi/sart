use std::{collections::BTreeMap, default, fmt::Debug, sync::Arc, time::Duration};

use futures::StreamExt;
use k8s_openapi::{
    api::core::v1::{Node, NodeSelector},
    apimachinery::pkg::apis::meta::v1::LabelSelector,
};
use kube::{
    api::{Api, ListParams},
    client::Client,
    core::DynamicObject,
    runtime::{
        controller::{Action, Controller},
        watcher::Config,
    },
    CustomResource, ResourceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    bgp::peer,
    context::{Context, State},
    error::Error, reconcilers::common::{get_match_labels, get_node_internal_addr}, speaker::speaker::Speaker,
};

use super::common::{error_policy, Endpoint};

pub(crate) const ASN_LABEL: &'static str = "asn.sart.terassyi.net";

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "ClusterBgp")]
#[kube(status = "ClusterBgpStatus")]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClusterBgpSpec {
    pub policy: Policy,
    pub peers: Option<Vec<peer::Peer>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct ClusterBgpStatus {}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Policy {
    pub node_selector: Option<LabelSelector>,
    pub id_selector: Option<IdSelector>,
    pub asn_selector: Option<AsnSelector>,
    pub endpoint: Endpoint,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AsnSelector {
    pub from: AsnSelectionType,
    pub asn: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema, PartialEq, Eq)]
pub(crate) enum AsnSelectionType {
    #[default]
    #[serde(rename = "asn")]
    Asn,
    #[serde(rename = "label")]
    Label,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct IdSelector {
    pub from: IdSelectionType,
    pub interface: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema, PartialEq, Eq)]
pub(crate) enum IdSelectionType {
    #[default]
    #[serde(rename = "label")]
    Label,
}

#[instrument(skip(ctx, resource), fields(trace_id))]
async fn reconcile(resource: Arc<ClusterBgp>, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!("ClusterBgp reconcile");

    let nodes: Api<Node> = Api::all(ctx.client.clone());

    let labels = get_match_labels(&resource);

    tracing::info!(labels=?labels,"Get ClusterBgp label selector");

    let matched_nodes = match labels {
        Some(labels) => {
            let selector = labels.join(",");
            tracing::info!(selector = selector, "matching selectors");
            let lp = ListParams::default().labels(&selector);
            nodes.list(&lp).await.map_err(Error::KubeError)?.items
        }
        None => {
            nodes
                .list(&ListParams::default())
                .await
                .map_err(Error::KubeError)?
                .items
        }
    };
    let node_names: Vec<String> = matched_nodes
        .iter()
        .map(|n| n.metadata.name.as_ref().unwrap().clone())
        .collect();

    tracing::info!(nodes=?node_names,name=resource.name_any(),"Matched nodes for ClusterBgp");


    // Configure BGP speakers
    for node in matched_nodes.iter() {
        tracing::info!(node=node.name_any(),"configure local BGP speaker");

        let addr = get_node_internal_addr(node).ok_or(Error::AddressNotFound)?;

        let asn: u32 = match &resource.spec.policy.asn_selector {
            Some(asn_selector) => {
                match asn_selector.from {
                    AsnSelectionType::Label => {
                        node.labels().get(ASN_LABEL)
                            .ok_or(Error::LabelMatchingError(format!("{} is not found", ASN_LABEL)))?
                            .parse()
                            .map_err(|_| Error::InvalidAsnValue)?
                    },
                    AsnSelectionType::Asn => {
                        asn_selector.asn.ok_or(Error::InvalidParameter("ASN must be specified".to_string()))?
                    }
                }

            },
            None => {
                node.labels().get(ASN_LABEL)
                    .ok_or(Error::LabelMatchingError(format!("{} is not found", ASN_LABEL)))?
                    .parse()
                    .map_err(|_| Error::InvalidAsnValue)?
            }
        };

        let endpoint = resource.spec.policy.endpoint.clone().host(&resource.name_any());
        let speaker_client = endpoint.create()?;

        let info = speaker_client.get_bgp_info().await?;


    }

    Ok(Action::requeue(Duration::from_secs(5)))
}

#[instrument(skip_all)]
pub(crate) async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube Client");

    let cluster_bgps = Api::<ClusterBgp>::all(client.clone());
    if let Err(e) = cluster_bgps.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Starting ClusterBgp reconciler");

    Controller::new(cluster_bgps, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconcile,
            error_policy::<ClusterBgp>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
