use super::address_pool::AddressType;
pub use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const ADDRESS_BLOCK_FINALIZER: &str = "addressblock.sart.terassyi.net/finalizer";
pub const ADDRESS_BLOCK_NODE_LABEL: &str = "addressblock.sart.terassyi.net/node";

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(
    group = "sart.terassyi.net",
    version = "v1alpha2",
    kind = "AddressBlock"
)]
#[kube(status = "AddressBlockStatus")]
#[kube(
    printcolumn = r#"{"name":"CIDR", "type":"string", "description":"CIDR of Address pool", "jsonPath":".spec.cidr"}"#,
    printcolumn = r#"{"name":"TYPE", "type":"string", "description":"Type of Address pool", "jsonPath":".spec.type"}"#,
    printcolumn = r#"{"name":"POOLREF", "type":"string", "description":"pool name", "jsonPath":".spec.poolRef"}"#,
    printcolumn = r#"{"name":"NODEREF", "type":"string", "description":"node name", "jsonPath":".spec.nodeRef"}"#,
    printcolumn = r#"{"name":"AGE", "type":"date", "description":"Date from created", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct AddressBlockSpec {
    pub cidr: String,
    pub r#type: AddressType,
    pub pool_ref: String,
    pub node_ref: Option<String>,
    pub auto_assign: bool, // default false
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct AddressBlockStatus {}
