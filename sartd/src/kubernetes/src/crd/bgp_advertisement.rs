use std::collections::BTreeMap;

use ipnet::IpNet;
pub use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::address_pool::AddressType;

pub const BGP_ADVERTISEMENT_FINALIZER: &str = "bgpadvertisement.sart.terassyi.net/finalizer";

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
pub struct BGPAdvertisementSpec {
    pub cidr: String,
    pub r#type: AddressType,
    pub protocol: Protocol,
    pub attrs: Option<Vec<String>>, // TODO: implement attributes
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct BGPAdvertisementStatus {
    pub peers: Option<BTreeMap<String, AdvertiseStatus>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema, PartialEq, Eq)]
pub enum AdvertiseStatus {
    #[default]
    NotAdvertised,
    Advertised,
    Withdraw,
}

impl std::fmt::Display for AdvertiseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdvertiseStatus::Advertised => write!(f, "advertised"),
            AdvertiseStatus::NotAdvertised => write!(f, "notadvertised"),
            AdvertiseStatus::Withdraw => write!(f, "withdraw"),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum Protocol {
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
