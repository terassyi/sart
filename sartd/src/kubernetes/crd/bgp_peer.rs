use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "BGPPeer")]
#[kube(status = "BGPPeerStatus")]
#[serde(rename_all = "camelCase")]
pub(crate) struct BGPPeerSpec {
	pub asn: u32,
	pub addr: String,

}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct BGPPeerStatus {}
