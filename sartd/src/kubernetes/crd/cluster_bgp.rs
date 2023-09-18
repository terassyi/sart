use std::collections::BTreeMap;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::bgp_peer::PeerConfig;

pub(crate) const CLUSTER_BGP_FINALIZER: &str = "clusterbgp.sart.terassyi.net/finalizer";
pub(crate) const ASN_LABEL: &str = "sart.terassyi.net/asn";
pub(crate) const ROUTER_ID_LABEL: &str = "sart.terassyi.net/router-id";

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "ClusterBGP")]
#[kube(status = "ClusterBGPStatus")]
#[kube(
    printcolumn = r#"{"name":"AGE", "type":"date", "description":"Date from created", "jsonPath":".metadata.creationTimestamp"}"#,
)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClusterBGPSpec {
    pub node_selector: Option<BTreeMap<String, String>>,
    pub asn_selector: AsnSelector,
    pub router_id_selector: RouterIdSelector,
    pub speaker: SpeakerConfig,
    pub peers: Option<Vec<PeerConfig>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct ClusterBGPStatus {}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeBGPConfig {
    pub asn_selector: AsnSelector,
    pub router_id_from: String,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AsnSelector {
    pub from: AsnSelectionType,
    pub asn: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AsnSelectionType {
    #[default]
    #[serde(rename = "asn")]
    Asn,
    #[serde(rename = "label")]
    Label,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RouterIdSelector {
    pub from: RouterIdSelectionType,
    pub router_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum RouterIdSelectionType {
    #[default]
    #[serde(rename = "internalAddress")]
    InternalAddress,
    #[serde(rename = "label")]
    Label,
    #[serde(rename = "routerId")]
    RouterId,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpeakerConfig {
    pub path: String,
    pub timeout: Option<u64>,
}
