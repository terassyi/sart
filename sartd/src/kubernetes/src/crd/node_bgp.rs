use core::fmt;

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::bgp_peer::BGPPeerSlim;

use super::cluster_bgp::SpeakerConfig;

pub const NODE_BGP_FINALIZER: &str = "nodebgp.sart.terassyi.net/finalizer";

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "NodeBGP")]
#[kube(status = "NodeBGPStatus")]
#[kube(
    printcolumn = r#"{"name":"ASN", "type":"integer", "description":"ASN of the local BGP speaker", "jsonPath":".spec.asn"}"#,
    printcolumn = r#"{"name":"ROUTERID", "type":"string", "description":"Router ID of the local BGP speaker", "jsonPath":".spec.routerId"}"#,
    printcolumn = r#"{"name":"BACKOFF", "type":"integer", "description":"Back off counter", "jsonPath":".status.backoff"}"#,
    printcolumn = r#"{"name":"STATUS", "type":"string", "description":"Status of a local speaker", "jsonPath":".status.conditions[-1:].status"}"#,
    printcolumn = r#"{"name":"AGE", "type":"date", "description":"Date from created", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct NodeBGPSpec {
    pub asn: u32,
    pub router_id: String,
    pub speaker: SpeakerConfig,
    pub peers: Option<Vec<BGPPeerSlim>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeBGPStatus {
    pub backoff: u32,
    pub cluster_bgp_refs: Option<Vec<String>>,
    pub conditions: Option<Vec<NodeBGPCondition>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema, PartialEq, Eq)]
pub struct NodeBGPCondition {
    pub status: NodeBGPConditionStatus,
    pub reason: NodeBGPConditionReason,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema, PartialEq, Eq)]
pub enum NodeBGPConditionStatus {
    Available,
    #[default]
    Unavailable,
}

impl std::fmt::Display for NodeBGPConditionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Available => write!(f, "available"),
            Self::Unavailable => write!(f, "unavailable"),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema, PartialEq, Eq)]
pub enum NodeBGPConditionReason {
    #[default]
    NotConfigured,
    Configured,
    InvalidConfiguration,
}
