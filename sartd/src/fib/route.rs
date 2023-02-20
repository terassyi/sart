use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use futures::TryStreamExt;
use netlink_packet_route::route::Nla;

use crate::proto;

use super::error::Error;

#[derive(Debug)]
pub(crate) struct RtClient {
    handler: rtnetlink::Handle,
}

impl RtClient {
    pub fn new(handler: rtnetlink::Handle) -> RtClient {
        RtClient { handler }
    }

    pub async fn get_routes(&self) -> Result<Vec<proto::sart::Route>, Error> {
        let routes = self.handler.route();
        let mut res = routes.get(rtnetlink::IpVersion::V4).execute();
		let mut proto_routes = Vec::new();
        while let Some(route) = res.try_next().await? {
            println!("{route:?}");
            let prefix = route.destination_prefix().ok_or(Error::FailedToGetPrefix)?;
            let prefix = ipnet::IpNet::new(prefix.0, prefix.1).map_err(|_| Error::FailedToGetPrefix)?;

            let source = match route.source_prefix() {
                Some((addr, _prefix_len)) => addr.to_string(),
                None => String::new()
            };

            let ad = AdministrativeDistance::from_protocol(Protocol::try_from(route.header.protocol)?, false);

            let mut proto_route = proto::sart::Route{
                table_id: route.header.table as u32, 
                afi: route.header.address_family as u32,
                destination: prefix.to_string(), 
                protocol: route.header.protocol as i32, 
                scope: route.header.scope as i32, 
                r#type: route.header.kind as i32, 
                next_hops: Vec::new(), 
                source, 
                ad: ad as i32, 
                priority : 20,
                ibgp: false, 
            };

            let mut oif = 0;
            for nla in route.nlas.iter() {
                match nla {
                    Nla::Priority(p) => {
                        proto_route.priority = *p;
                    },
                    Nla::MultiPath(m) => {},
                    Nla::Gateway(g) => {
                        let gateway = if g.len() == 4 {
                            let a: [u8; 4] = g.to_vec().try_into().map_err(|_| Error::FailedToParseAddress)?;
                            IpAddr::V4(Ipv4Addr::from(a))
                        } else if g.len() == 16 {
                            let a: [u8; 16] = g.to_vec().try_into().map_err(|_| Error::FailedToParseAddress)?;
                            IpAddr::V6(Ipv6Addr::from(a))

                        } else {
                            return Err(Error::FailedToGetPrefix)
                        };
                        let next_hop = proto::sart::NextHop{
                            gateway: gateway.to_string(), 
                            weight: 1, 
                            flags: 0, 
                            interface: 0
                        };
                        proto_route.next_hops.push(next_hop);
                    },
                    Nla::Oif(i) => oif = *i,
                    _ => {},
                }
            }
        }
        Ok(proto_routes)
    }
}


pub(crate) enum AdministrativeDistance {
    Connected  = 0,
    Static     = 1,
    EBGP       = 20,
    OSPF       = 110,
    RIP        = 120,
    IBGP       = 200,
}

impl AdministrativeDistance {
    fn from_protocol(protocol: Protocol, internal: bool) -> AdministrativeDistance {
        match protocol {
            Protocol::Unspec | Protocol::Redirect | Protocol::Kernel | Protocol::Boot => AdministrativeDistance::Connected,
            Protocol::Static => AdministrativeDistance::Static,
            Protocol::Bgp => {
                if internal {
                    AdministrativeDistance::IBGP
                } else {
                    AdministrativeDistance::EBGP
                }
            },
            Protocol::Ospf => AdministrativeDistance::OSPF,
            Protocol::Rip => AdministrativeDistance::RIP
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
            _ => Err(Error::InvalidADValue)
        }
    }
}

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
            _ => Err(Error::InvalidProtocol)
        }
    }
}

fn nlas_to_nexthop<'a>(next_hops: &'a mut Vec<proto::sart::NextHop>, nlas: &'a Vec<Nla>) -> Result<&'a mut Vec<proto::sart::NextHop>, Error> {
    let mut next_hop = proto::sart::NextHop{
        gateway: String::new(), 
        weight: 1, 
        flags: 0, 
        interface: 0
    };
    for nla in nlas.iter() {
        match nla {
            Nla::Gateway(g) => {
                let gateway = if g.len() == 4 {
                    let a: [u8; 4] = g.to_vec().try_into().map_err(|_| Error::FailedToParseAddress)?;
                    IpAddr::V4(Ipv4Addr::from(a))
                } else if g.len() == 16 {
                    let a: [u8; 16] = g.to_vec().try_into().map_err(|_| Error::FailedToParseAddress)?;
                    IpAddr::V6(Ipv6Addr::from(a))

                } else {
                    return Err(Error::FailedToGetPrefix)
                };

                next_hop.gateway = gateway.to_string();
            },
            Nla::Oif(i) => next_hop.interface = *i,
            Nla::MultiPath(multi_paths) => {
                for m in multi_paths.iter() {
                }
            }
            _ => {},
        }
    }

    next_hops.push(next_hop);

    Ok(next_hops)
}
