use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use futures::TryStreamExt;
use ipnet::IpNet;
use netlink_packet_route::{route::Nla, RouteMessage};
// use netlink_packet_route::route::{NextHop, NextHopFlags, Nla};

use crate::proto;

use super::error::Error;

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct Route {
    pub destination: IpNet,
    pub version: rtnetlink::IpVersion,
    pub protocol: Protocol,
    pub scope: Scope,
    pub kind: Kind,
    pub next_hops: Vec<NextHop>,
    pub source: Option<IpAddr>,
    pub priority: u32,
    pub ad: AdministrativeDistance,
    pub table: u8,
}

impl Default for Route {
    fn default() -> Self {
        Self {
            destination: "0.0.0.0/0".parse().unwrap(),
            version: rtnetlink::IpVersion::V4,
            protocol: Protocol::Unspec,
            scope: Scope::Universe,
            kind: Kind::UnspecType,
            next_hops: Vec::new(),
            source: None,
            priority: 0,
            ad: AdministrativeDistance::Static,
            table: 0,
        }
    }
}

impl Ord for Route {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ad.cmp(&other.ad)
    }
}

impl PartialOrd for Route {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl TryFrom<RouteMessage> for Route {
    type Error = Error;
    fn try_from(msg: RouteMessage) -> Result<Self, Self::Error> {
        let destination = match msg.destination_prefix() {
            Some((addr, prefix_len)) => IpNet::new(addr, prefix_len),
            None => return Err(Error::FailedToGetPrefix),
        }
        .map_err(|_| Error::FailedToGetPrefix)?;
        let version = match msg.header.address_family {
            2 => rtnetlink::IpVersion::V4,
            10 => rtnetlink::IpVersion::V6,
            _ => return Err(Error::InvalidIpVersion),
        };

        let protocol = Protocol::try_from(msg.header.protocol)?;
        let mut route = Route {
            destination,
            version,
            protocol,
            scope: Scope::try_from(msg.header.scope as i32)?,
            kind: Kind::try_from(msg.header.kind as i32)?,
            next_hops: Vec::new(),
            source: msg.source_prefix().map(|(addr, _)| addr),
            priority: 0,
            ad: AdministrativeDistance::from_protocol(protocol, false),
            table: msg.header.table,
        };

        if msg.nlas.iter().any(|nla| matches!(&nla, Nla::MultiPath(_))) {
            // multi path
            for nla in msg.nlas.iter() {
                match nla {
                    Nla::Priority(p) => route.priority = *p,
                    Nla::MultiPath(hops) => {
                        for h in hops.iter() {
                            let mut next_hop = NextHop {
                                gateway: "0.0.0.0".parse().unwrap(),
                                weight: h.hops as u32,
                                flags: NextHopFlags::try_from(h.flags.bits() as i32)?,
                                interface: h.interface_id,
                            };
                            for nnla in h.nlas.iter() {
                                match nnla {
                                    Nla::Gateway(g) => {
                                        next_hop.gateway = parse_ipaddr(g)?;
                                    }
                                    Nla::Oif(i) => next_hop.interface = *i,
                                    _ => {}
                                }
                            }
                            route.next_hops.push(next_hop);
                        }
                    }
                    _ => {}
                }
            }
        } else {
            let mut next_hop = NextHop {
                gateway: "0.0.0.0".parse().unwrap(),
                weight: 0,
                flags: NextHopFlags::Empty,
                interface: 0,
            };
            for nla in msg.nlas.iter() {
                match nla {
                    Nla::Priority(p) => route.priority = *p,
                    Nla::Gateway(g) => next_hop.gateway = parse_ipaddr(g)?,
                    Nla::PrefSource(p) => next_hop.gateway = parse_ipaddr(p)?,
                    Nla::Oif(i) => next_hop.interface = *i,
                    _ => {}
                }
            }
            route.next_hops.push(next_hop);
        }
        Ok(route)
    }
}

impl TryFrom<&crate::proto::sart::Route> for Route {
    type Error = Error;
    fn try_from(value: &proto::sart::Route) -> Result<Self, Self::Error> {
        let dst: IpNet = value
            .destination
            .parse()
            .map_err(|_| Error::FailedToParseAddress)?;
        let ip_version = match value.version {
            2 => rtnetlink::IpVersion::V4,
            10 => rtnetlink::IpVersion::V6,
            _ => return Err(Error::InvalidIpVersion),
        };
        let mut next_hops = Vec::new();
        for n in value.next_hops.iter() {
            next_hops.push(NextHop::try_from(n)?);
        }
        let source = match value.source.parse() {
            Ok(a) => Some(a),
            Err(_) => None,
        };

        Ok(Route {
            destination: dst,
            version: ip_version,
            protocol: Protocol::try_from(value.protocol as u8)?,
            scope: Scope::try_from(value.scope)?,
            kind: Kind::try_from(value.r#type)?,
            next_hops,
            source,
            priority: value.priority,
            ad: AdministrativeDistance::try_from(value.ad as u8)?,
            table: value.table as u8,
        })
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone)]
pub(crate) struct NextHop {
    pub gateway: IpAddr,
    pub weight: u32,
    pub flags: NextHopFlags,
    pub interface: u32,
}

impl Default for NextHop {
    fn default() -> Self {
        Self {
            gateway: "0.0.0.0".parse().unwrap(),
            weight: 0,
            flags: NextHopFlags::Empty,
            interface: 0,
        }
    }
}

impl TryFrom<&proto::sart::NextHop> for NextHop {
    type Error = Error;
    fn try_from(value: &proto::sart::NextHop) -> Result<Self, Self::Error> {
        let gateway = value
            .gateway
            .parse()
            .map_err(|_| Error::FailedToParseAddress)?;

        Ok(NextHop {
            gateway,
            weight: value.weight,
            flags: NextHopFlags::try_from(value.flags)?,
            interface: value.interface,
        })
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone, Copy)]
pub(crate) enum NextHopFlags {
    Empty = 0,
    Dead = 1,
    Pervasive = 2,
    Onlink = 3,
    Offload = 4,
    Linkdown = 16,
    Unresolved = 32,
}

impl TryFrom<i32> for NextHopFlags {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(NextHopFlags::Empty),
            1 => Ok(NextHopFlags::Dead),
            2 => Ok(NextHopFlags::Pervasive),
            3 => Ok(NextHopFlags::Onlink),
            4 => Ok(NextHopFlags::Offload),
            16 => Ok(NextHopFlags::Linkdown),
            32 => Ok(NextHopFlags::Unresolved),
            _ => Err(Error::InvalidNextHopFlag),
        }
    }
}

impl Into<u8> for NextHopFlags {
    fn into(self) -> u8 {
        match self {
            NextHopFlags::Empty => 0,
            NextHopFlags::Dead => 1,
            NextHopFlags::Pervasive => 2,
            NextHopFlags::Onlink => 3,
            NextHopFlags::Offload => 4,
            NextHopFlags::Linkdown => 16,
            NextHopFlags::Unresolved => 32,
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum Scope {
    Universe = 0,
    Site = 200,
    Link = 253,
    Host = 254,
    Nowhere = 255,
}

impl TryFrom<i32> for Scope {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Scope::Universe),
            200 => Ok(Scope::Site),
            253 => Ok(Scope::Link),
            254 => Ok(Scope::Host),
            255 => Ok(Scope::Nowhere),
            _ => Err(Error::InvalidScope),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum Kind {
    UnspecType = 0,
    Unicast = 1,
    Local = 2,
    Broadcast = 3,
    Anycast = 4,
    Multicast = 5,
    Blackhole = 6,
    Unreachable = 7,
    Prohibit = 8,
    Throw = 9,
    Nat = 10,
}

impl TryFrom<i32> for Kind {
    type Error = Error;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Kind::UnspecType),
            1 => Ok(Kind::Unicast),
            2 => Ok(Kind::Local),
            3 => Ok(Kind::Broadcast),
            4 => Ok(Kind::Anycast),
            5 => Ok(Kind::Multicast),
            6 => Ok(Kind::Blackhole),
            7 => Ok(Kind::Unreachable),
            8 => Ok(Kind::Prohibit),
            9 => Ok(Kind::Throw),
            10 => Ok(Kind::Nat),
            _ => Err(Error::InvalidType),
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord, Clone, Copy)]
pub(crate) enum AdministrativeDistance {
    Connected = 0,
    Static = 1,
    EBGP = 20,
    OSPF = 110,
    RIP = 120,
    IBGP = 200,
}

impl std::fmt::Display for AdministrativeDistance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected => write!(f, "Connected"),
            Self::Static => write!(f, "Static"),
            Self::EBGP => write!(f, "EBGP"),
            Self::OSPF => write!(f, "OSPF"),
            Self::RIP => write!(f, "RIP"),
            Self::IBGP => write!(f, "IBGP"),
        }
    }
}

impl AdministrativeDistance {
    pub(crate) fn from_protocol(protocol: Protocol, internal: bool) -> AdministrativeDistance {
        match protocol {
            Protocol::Unspec | Protocol::Redirect | Protocol::Kernel | Protocol::Boot => {
                AdministrativeDistance::Connected
            }
            Protocol::Static => AdministrativeDistance::Static,
            Protocol::Bgp => {
                if internal {
                    AdministrativeDistance::IBGP
                } else {
                    AdministrativeDistance::EBGP
                }
            }
            Protocol::Ospf => AdministrativeDistance::OSPF,
            Protocol::Rip => AdministrativeDistance::RIP,
        }
    }
}

impl TryFrom<u8> for AdministrativeDistance {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Connected),
            1 => Ok(Self::Static),
            20 => Ok(Self::EBGP),
            110 => Ok(Self::OSPF),
            120 => Ok(Self::RIP),
            200 => Ok(Self::IBGP),
            _ => Err(Error::InvalidADValue),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum Protocol {
    Unspec = 0,
    Redirect = 1,
    Kernel = 2,
    Boot = 3,
    Static = 4,
    Bgp = 186,
    Ospf = 188,
    Rip = 189,
}

impl Into<u8> for Protocol {
    fn into(self) -> u8 {
        match self {
            Protocol::Unspec => 0,
            Protocol::Redirect => 1,
            Protocol::Kernel => 2,
            Protocol::Boot => 3,
            Protocol::Static => 4,
            Protocol::Bgp => 186,
            Protocol::Ospf => 188,
            Protocol::Rip => 189,
        }
    }
}

impl TryFrom<u8> for Protocol {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unspec),
            1 => Ok(Self::Redirect),
            2 => Ok(Self::Kernel),
            3 => Ok(Self::Boot),
            4 => Ok(Self::Static),
            186 => Ok(Self::Bgp),
            188 => Ok(Self::Ospf),
            189 => Ok(Self::Rip),
            _ => Err(Error::InvalidProtocol),
        }
    }
}

pub(crate) fn ip_version_from(val: u32) -> Result<rtnetlink::IpVersion, Error> {
    match val {
        2 => Ok(rtnetlink::IpVersion::V4),
        10 => Ok(rtnetlink::IpVersion::V6),
        _ => Err(Error::InvalidProtocol),
    }
}

pub(crate) fn ip_version_into(ver: &rtnetlink::IpVersion) -> u8 {
    match ver {
        rtnetlink::IpVersion::V4 => 2,
        rtnetlink::IpVersion::V6 => 10,
    }
}

pub(crate) fn parse_ipaddr(data: &Vec<u8>) -> Result<IpAddr, Error> {
    if data.len() == 4 {
        let a: [u8; 4] = data
            .to_vec()
            .try_into()
            .map_err(|_| Error::FailedToParseAddress)?;
        Ok(IpAddr::V4(Ipv4Addr::from(a)))
    } else if data.len() == 16 {
        let a: [u8; 16] = data
            .to_vec()
            .try_into()
            .map_err(|_| Error::FailedToParseAddress)?;
        Ok(IpAddr::V6(Ipv6Addr::from(a)))
    } else {
        Err(Error::FailedToGetPrefix)
    }
}
