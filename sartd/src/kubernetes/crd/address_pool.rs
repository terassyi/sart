


use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};



pub(crate) const ADDRESS_POOL_FINALIZER: &str = "addresspool.sart.terassyi.net/finalizer";

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "AddressPool")]
#[kube(status = "AddressPoolStatus")]
#[kube(
    printcolumn = r#"{"name":"AGE", "type":"date", "description":"Date from created", "jsonPath":".metadata.creationTimestamp"}"#,
)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AddressPoolSpec {
	pub cidr: String,
	pub r#type: AddressType,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct AddressPoolStatus {}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum AddressType {
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
