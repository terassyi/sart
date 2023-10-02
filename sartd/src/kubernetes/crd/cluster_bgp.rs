use k8s_openapi::api::core::v1::NodeSelector;
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

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "ClusterBGP")]
#[kube(status = "ClusterBGPStatus")]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClusterBGPSpec {
    // pub policy: Policy,
    // pub peers: Option<Vec<peer::Peer>>,
    pub node_selector: NodeSelector,
    pub asn_selector: AsnSelector,
    pub router_id_selector: RouterIdSelector,
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
    #[serde(rename = "label")]
    Label,
}
