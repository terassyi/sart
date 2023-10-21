use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::bgp_peer::BGPPeerSlim;

use super::cluster_bgp::SpeakerConfig;

pub(crate) const NODE_BGP_FINALIZER: &str = "clusterbgp.sart.terassyi.net/finalizer";

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "NodeBGP")]
#[kube(status = "NodeBGPStatus")]
#[kube(
    printcolumn = r#"{"name":"ASN", "type":"integer", "description":"ASN of the local BGP speaker", "jsonPath":".spec.asn"}"#,
    printcolumn = r#"{"name":"ROUTERID", "type":"string", "description":"Router ID of the local BGP speaker", "jsonPath":".spec.routerId"}"#,
    printcolumn = r#"{"name":"AGE", "type":"date", "description":"Date from created", "jsonPath":".metadata.creationTimestamp"}"#,
    printcolumn = r#"{"name":"STATUS", "type":"string", "description":"Status of a local speaker", "jsonPath":".status.conditions[-1:].status"}"#
)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeBGPSpec {
    pub asn: u32,
    pub router_id: String,
    pub speaker: SpeakerConfig,
    pub peers: Option<Vec<BGPPeerSlim>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct NodeBGPStatus {
    pub conditions: Option<Vec<NodeBGPCondition>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct NodeBGPCondition {
    pub status: NodeBGPConditionStatus,
    pub reason: NodeBGPConditionReason,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema, PartialEq, Eq)]
pub(crate) enum NodeBGPConditionStatus {
    Available,
    #[default]
    Unavailable,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema, PartialEq, Eq)]
pub(crate) enum NodeBGPConditionReason {
    #[default]
    NotConfigured,
    Configured,
    InvalidConfiguration,
}
