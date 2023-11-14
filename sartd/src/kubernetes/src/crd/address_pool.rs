use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const ADDRESS_POOL_FINALIZER: &str = "addresspool.sart.terassyi.net/finalizer";
pub const ADDRESS_POOL_ANNOTATION: &str = "sart.terassyi.net/addresspool";
pub const LOADBALANCER_ADDRESS_ANNOTATION: &str = "sart.terassyi.net/loadBalancerIPs";

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(
    group = "sart.terassyi.net",
    version = "v1alpha2",
    kind = "AddressPool"
)]
#[kube(status = "AddressPoolStatus")]
#[kube(
    printcolumn = r#"{"name":"CIDR", "type":"string", "description":"CIDR of Address pool", "jsonPath":".spec.cidr"}"#,
    printcolumn = r#"{"name":"TYPE", "type":"string", "description":"Type of Address pool", "jsonPath":".spec.type"}"#,
    printcolumn = r#"{"name":"BLOCKSIZE", "type":"integer", "description":"block size of CIDR", "jsonPath":".spec.blockSize"}"#,
    printcolumn = r#"{"name":"AGE", "type":"date", "description":"Date from created", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct AddressPoolSpec {
    pub cidr: String,
    pub r#type: AddressType,
    pub alloc_type: Option<AllocationType>,
    pub block_size: u32,
    pub auto_assign: Option<bool>, // default false
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct AddressPoolStatus {
    pub blocks: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Clone, Copy, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AddressType {
    #[default]
    #[serde(rename = "service")]
    Service,
}

impl std::fmt::Display for AddressType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Service => write!(f, "service"),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Copy, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum AllocationType {
    #[default]
    #[serde(rename = "bit")]
    Bit,
}
