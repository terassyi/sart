use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(
    group = "sart.terassyi.net",
    version = "v1alpha2",
    kind = "BGPPeerTemplate"
)]
#[kube(status = "BGPPeerTemplateStatus")]
#[kube(
    printcolumn = r#"{"name":"AGE", "type":"date", "description":"Date from created", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct BGPPeerTemplateSpec {
    pub asn: Option<u32>,
    pub addr: Option<String>,
    pub capabilities: Option<Vec<String>>,
    pub hold_time: Option<u32>,
    pub keepalive_time: Option<u32>,
    pub groups: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct BGPPeerTemplateStatus {}
