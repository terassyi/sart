use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const BLOCK_REQUEST_FINALIZER: &str = "blockrequest.sart.terassyi.net/finalizer";

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(
	group = "sart.terassyi.net",
	version = "v1alpha2",
	kind = "BlockRequest"
)]
#[kube(status = "BlockRequestStatus")]
#[kube(
    printcolumn = r#"{"name":"POOL", "type":"string", "description":"Address pool name", "jsonPath":".spec.pool"}"#,
    printcolumn = r#"{"name":"NODE", "type":"string", "description":"Node name", "jsonPath":".spec.node"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct BlockRequestSpec {
	pub pool: String,
	pub node: String,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct BlockRequestStatus {}
