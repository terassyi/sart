use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};



#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "NodeBGP")]
#[kube(status = "NodeBGPStatus")]
#[serde(rename_all = "camelCase")]
pub(crate) struct NodeBGPSpec {
	pub asn: u32,
	pub router_id: String,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct NodeBGPStatus {}
