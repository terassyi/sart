use std::collections::BTreeMap;

use kube::core::ObjectMeta;
use kube::{Api, CustomResource, ResourceExt};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::bgp_peer_template::BGPPeerTemplate;
use super::node_bgp::NodeBGP;

use super::cluster_bgp::SpeakerConfig;

use super::bgp_peer_template::BGPPeerTemplateSpec;
use super::error::Error;

pub(crate) const BGP_PEER_FINALIZER: &str = "bgppeer.sart.terassyi.net/finalizer";
pub(crate) const BGP_PEER_NODE_LABEL: &str = "bgppeer.sart.terassyi.net/node";
pub(crate) const PEER_GROUP_ANNOTATION: &str = "bgppeer.sart.terassyi.net/group";

#[derive(CustomResource, Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
// #[cfg_attr(test, derive(Default))]
#[kube(group = "sart.terassyi.net", version = "v1alpha2", kind = "BGPPeer")]
#[kube(status = "BGPPeerStatus")]
#[kube(
    printcolumn = r#"{"name":"ASN", "type":"integer", "description":"ASN of the remote BGP speaker", "jsonPath":".spec.asn"}"#,
    printcolumn = r#"{"name":"ADDRESS", "type":"string", "description":"Address of the remote BGP speaker", "jsonPath":".spec.addr"}"#,
    printcolumn = r#"{"name":"NODEBGP", "type":"string", "description":"Name of the NodeBGP that has local BGP speaker information", "jsonPath":".spec.nodeBGPRef"}"#,
    printcolumn = r#"{"name":"STATUS", "type":"string", "description":"Status of a local speaker", "jsonPath":".status.conditions[-1:].status"}"#,
    printcolumn = r#"{"name":"AGE", "type":"date", "description":"Date from created", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BGPPeerSpec {
    pub asn: u32,
    pub addr: String,
    pub hold_time: Option<u32>,
    pub keepalive_time: Option<u32>,
    // TODO: this should be the vector of capability struct
    pub capabilities: Option<Vec<String>>,
    #[serde(rename = "nodeBGPRef")]
    pub node_bgp_ref: String,
    pub speaker: SpeakerConfig,
    pub groups: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct BGPPeerStatus {
    pub conditions: Option<Vec<BGPPeerCondition>>,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub(crate) struct BGPPeerCondition {
    pub status: BGPPeerConditionStatus,
    pub reason: String,
}

#[derive(Deserialize, Serialize, Clone, Copy, Default, Debug, JsonSchema, PartialEq, Eq)]
pub(crate) enum BGPPeerConditionStatus {
    Unknown = 0,
    #[default]
    Idle = 1,
    Active = 2,
    Connect = 3,
    OpenSent = 4,
    OpenConfirm = 5,
    Established = 6,
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BGPPeerSlim {
    pub name: String,
    pub spec: BGPPeerSpec,
}

impl BGPPeerSlim {
    pub fn from(&self, nb: &NodeBGP) -> BGPPeer {
        BGPPeer {
            metadata: ObjectMeta {
                name: Some(self.name.clone()),
                labels: Some(BTreeMap::from([(
                    BGP_PEER_NODE_LABEL.to_string(),
                    nb.name_any(),
                )])),
                ..Default::default()
            },
            spec: BGPPeerSpec {
                asn: self.spec.asn,
                addr: self.spec.addr.clone(),
                hold_time: self.spec.hold_time,
                keepalive_time: self.spec.keepalive_time,
                capabilities: self.spec.capabilities.clone(),
                node_bgp_ref: nb.name_any(),
                speaker: nb.spec.speaker.clone(),
                groups: self.spec.groups.clone(),
            },
            status: None,
        }
    }

    pub fn into(bp: &BGPPeer) -> BGPPeerSlim {
        BGPPeerSlim {
            name: bp.name_any(),
            spec: BGPPeerSpec {
                asn: bp.spec.asn,
                addr: bp.spec.addr.clone(),
                hold_time: bp.spec.hold_time,
                keepalive_time: bp.spec.keepalive_time,
                capabilities: bp.spec.capabilities.clone(),
                groups: bp.spec.groups.clone(),
                ..Default::default() // omit
            },
        }
    }

    // match_groups() returns true if BGPPeerSlim.spec.groups contains either of groups given as an argument.
    // If BGPPeerSlim.spec.groups is empty(None) always returns true.
    // And if targets is empty, this returns true as well.
    pub fn match_groups(&self, targets: &[&str]) -> bool {
        if let Some(groups) = &self.spec.groups {
            if targets.is_empty() {
                return true;
            }
            for g in groups.iter() {
                if targets.contains(&g.as_str()) {
                    return true;
                }
            }
            false
        } else {
            true
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PeerConfig {
    pub peer_template_ref: Option<String>,
    // this fields override peer template if specified
    pub peer_config: Option<BGPPeerTemplateSpec>,
    #[serde(rename = "nodeBGPSelector")]
    pub node_bgp_selector: Option<BTreeMap<String, String>>,
}

impl PeerConfig {
    pub(crate) async fn to_peer(
        &self,
        node_name: &str,
        peer_templ_api: &Api<BGPPeerTemplate>,
    ) -> Result<BGPPeerSlim, Error> {
        let mut bp = BGPPeerSlim::default();

        if let Some(templ) = &self.peer_template_ref {
            let pt = peer_templ_api.get(templ).await.map_err(Error::KubeError)?;
            if let Some(a) = pt.spec.asn {
                bp.spec.asn = a;
            }
            if let Some(a) = pt.spec.addr {
                bp.spec.addr = a.clone();
            }

            bp.spec.hold_time = pt.spec.hold_time;
            bp.spec.keepalive_time = pt.spec.keepalive_time;
            bp.spec.groups = pt.spec.groups;
            bp.spec.capabilities = pt.spec.capabilities;
        }

        if let Some(o) = &self.peer_config {
            if let Some(a) = o.asn {
                bp.spec.asn = a;
            }
            if let Some(a) = &o.addr {
                bp.spec.addr = a.clone();
            }
        }

        let name = match &self.peer_template_ref {
            Some(n) => format!("{}-{}-{}-{}", n, node_name, bp.spec.asn, bp.spec.addr),
            None => format!("{}-{}-{}", node_name, bp.spec.asn, bp.spec.addr),
        };

        bp.name = name;

        Ok(bp)
    }
}

impl BGPPeerSpec {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.asn == 0 {
            return Err(Error::ValidationError("Invalid ASN".to_string()));
        }
        if self.addr.is_empty() {
            return Err(Error::ValidationError(
                "Invalid remote peer address".to_string(),
            ));
        }
        Ok(())
    }
}

impl TryFrom<i32> for BGPPeerConditionStatus {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Idle),
            2 => Ok(Self::Connect),
            3 => Ok(Self::Active),
            4 => Ok(Self::OpenSent),
            5 => Ok(Self::OpenConfirm),
            6 => Ok(Self::Established),
            _ => Err(Error::InvalidPeerState(value)),
        }
    }
}
