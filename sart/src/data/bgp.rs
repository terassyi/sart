use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    proto::{self},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BgpInfo {
    pub asn: u32,
    pub router_id: String,
    pub port: u32,
}

impl From<&proto::sart::BgpInfo> for BgpInfo {
    fn from(value: &proto::sart::BgpInfo) -> Self {
        Self {
            asn: value.asn,
            router_id: value.router_id.clone(),
            port: value.port,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub name: String,
    pub address: String,
    pub asn: u32,
    pub router_id: String,
    pub state: String,
    pub uptime: Option<String>,
    pub hold_time: u32,
    pub keep_alive_time: u32,
    pub families: Vec<Family>,
    pub capabilities: Vec<Capability>, // TODO: implement
    pub send_counter: Option<MessageCounter>,
    pub receive_counter: Option<MessageCounter>,
}

impl TryFrom<&proto::sart::Peer> for Peer {
    type Error = Error;
    fn try_from(value: &proto::sart::Peer) -> Result<Self, Self::Error> {
        let state = match value.state {
            1 => "Idle",
            2 => "Active",
            3 => "Connect",
            4 => "OpenSent",
            5 => "OpenConfirm",
            6 => "Established",
            _ => return Err(Error::InvalidBGPState(value.state as u32)),
        };

        Ok(Self {
            name: value.name.clone(),
            address: value.address.clone(),
            asn: value.asn,
            router_id: value.router_id.clone(),
            state: state.to_string(),
            uptime: value.uptime.as_ref().map(|u| u.to_string()).clone(),
            hold_time: value.hold_time,
            keep_alive_time: value.keepalive_time,
            families: value.families.iter().map(Family::from).collect(),
            capabilities: Vec::new(),
            send_counter: value.send_counter.clone().map(MessageCounter::from),
            receive_counter: value.recv_counter.clone().map(MessageCounter::from),
        })
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peers {
    pub peers: Vec<Peer>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Family {
    pub afi: String,
    pub safi: String,
}

impl From<&proto::sart::AddressFamily> for Family {
    fn from(value: &proto::sart::AddressFamily) -> Self {
        let afi = match value.afi {
            1 => "ipv4",
            2 => "ipv6",
            _ => "unknown",
        }
        .to_string();
        let safi = match value.safi {
            1 => "unicast",
            2 => "multicast",
            _ => "unknown",
        }
        .to_string();
        Self { afi, safi }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageCounter {
    pub open: u32,
    pub update: u32,
    pub keep_alive: u32,
    pub notification: u32,
    pub route_refresh: u32,
}

impl From<proto::sart::MessageCounter> for MessageCounter {
    fn from(value: proto::sart::MessageCounter) -> Self {
        Self {
            open: value.open,
            update: value.update,
            keep_alive: value.keepalive,
            notification: value.notification,
            route_refresh: value.route_refresh,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Capability {
    MultiProtocol(MultiProtocol),
    RouteRefresh(RouteRefresh),
    GracefulRestart(GracefulRestart),
    FourOctedASN(FourOctedASN),
    EnhancedRouteRefresh(EnhancedRouteRefresh),
    Unknown(Unknown),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiProtocol {
    pub family: Family,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteRefresh {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GracefulRestartTuple {
    pub family: Family,
    pub flags: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GracefulRestart {
    pub flags: u32,
    pub time: u32,
    pub tuples: Vec<GracefulRestartTuple>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FourOctedASN {
    pub asn: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnhancedRouteRefresh {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Unknown {
    pub code: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Paths {
    pub paths: Vec<Path>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Path {
    pub nlri: String,
    pub family: Option<Family>,
    pub origin: u32,
    pub next_hops: Vec<String>,
    pub segments: Vec<AsSegment>,
    pub local_pref: u32,
    pub med: u32,
    pub peer_asn: u32,
    pub peer_addr: String,
    pub best: bool,
    pub timestamp: Option<String>,
}

impl From<&proto::sart::Path> for Path {
    fn from(value: &proto::sart::Path) -> Self {
        Self {
            nlri: value.nlri.clone(),
            family: value.family.clone().map(|f| Family::from(&f)),
            origin: value.origin,
            next_hops: value.next_hops.clone(),
            segments: value.segments.iter().map(AsSegment::from).collect(),
            local_pref: value.local_pref,
            med: value.med,
            peer_asn: value.peer_asn,
            peer_addr: value.peer_addr.clone(),
            best: value.best,
            timestamp: value.timestamp.clone().map(|t| t.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AsSegment {
    pub r#type: String,
    pub elm: Vec<u32>,
}

impl From<&proto::sart::AsSegment> for AsSegment {
    fn from(value: &proto::sart::AsSegment) -> Self {
        Self {
            r#type: match value.r#type {
                1 => "set".to_string(),
                2 => "sequence".to_string(),
                _ => "unknown".to_string(),
            },
            elm: value.elm.clone(),
        }
    }
}
