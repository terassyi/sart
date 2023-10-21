use ipnet::IpNet;
use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::address_pool::AddressType;

pub(crate) const BGP_ADVERTISEMENT_FINALIZER: &str = "bgpadvertisement.sart.terassyi.net/finalizer";

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(
    group = "sart.terassyi.net",
    version = "v1alpha2",
    kind = "BGPAdvertisement"
)]
#[kube(status = "BGPAdvertisementStatus")]
#[kube(namespaced)]
#[kube(
    printcolumn = r#"{"name":"CIDR", "type":"string", "description":"Advertised CIDR", "jsonPath":".spec.cidr"}"#,
    printcolumn = r#"{"name":"TYPE", "type":"string", "description":"Type of advertised CIDR", "jsonPath":".spec.type"}"#,
    printcolumn = r#"{"name":"PROTOCOL", "type":"string", "description":"Type of advertised CIDR", "jsonPath":".spec.protocol"}"#,
    printcolumn = r#"{"name":"AGE", "type":"date", "description":"Date from created", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BGPAdvertisementSpec {
    pub cidr: String,
    pub r#type: AddressType,
    pub protocol: Protocol,
    pub attrs: Option<Vec<String>>, // TODO: implement attributes
    pub peers: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct BGPAdvertisementStatus {}


#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Protocol {
    #[default]
    #[serde(rename = "ipv4")]
    IPv4,
    #[serde(rename = "ipv6")]
    IPv6,
}

impl From<&IpNet> for Protocol {
    fn from(value: &IpNet) -> Self {
        match value {
            IpNet::V4(_) => Protocol::IPv4,
            IpNet::V6(_) => Protocol::IPv6,
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IPv4 => write!(f, "ipv4"),
            Self::IPv6 => write!(f, "ipv6"),
        }
    }
}
