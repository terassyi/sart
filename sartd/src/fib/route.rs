use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use futures::TryStreamExt;
use ipnet::IpNet;
use netlink_packet_route::{route::Nla, RouteMessage, RouteHeader, RouteFlags};

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

    #[tracing::instrument(skip(self))]
    pub async fn list_routes(
        &self,
        table: u32,
        ip_version: rtnetlink::IpVersion,
    ) -> Result<Vec<proto::sart::Route>, Error> {
        let routes = self.handler.route();
        let mut res = routes.get(ip_version.clone()).execute();

        tracing::info!("get routes");

        let mut proto_routes = Vec::new();
        while let Some(route) = res.try_next().await? {
            if route.header.table != table as u8
                || route.header.address_family != ip_version_into(&ip_version)
            {
                continue;
            }
            let prefix = match route.destination_prefix() {
                Some(prefix) => {
                    let prefix = ipnet::IpNet::new(prefix.0, prefix.1)
                        .map_err(|_| Error::FailedToGetPrefix)?;
                    prefix.to_string()
                }
                None => "0.0.0.0".to_string(),
            };
            let source = match route.source_prefix() {
                Some((addr, _prefix_len)) => addr.to_string(),
                None => String::new(),
            };

            let ad = AdministrativeDistance::from_protocol(
                Protocol::try_from(route.header.protocol)?,
                false,
            );

            let mut proto_route = proto::sart::Route {
                table_id: route.header.table as u32,
                ip_version: route.header.address_family as i32,
                destination: prefix,
                protocol: route.header.protocol as i32,
                scope: route.header.scope as i32,
                r#type: route.header.kind as i32,
                next_hops: Vec::new(),
                source,
                ad: ad as i32,
                priority: 20,
                ibgp: false,
            };

            if route
                .nlas
                .iter()
                .any(|nla| matches!(&nla, Nla::MultiPath(_)))
            {
                // multi path
                for nla in route.nlas.iter() {
                    match nla {
                        Nla::Priority(p) => proto_route.priority = *p,
                        Nla::MultiPath(hops) => {
                            for h in hops.iter() {
                                let mut next_hop = next_hop_default();
                                for nnla in h.nlas.iter() {
                                    match nnla {
                                        Nla::Gateway(g) => {
                                            next_hop.gateway = parse_ipaddr(g)?.to_string()
                                        }
                                        Nla::Oif(i) => next_hop.interface = *i,
                                        _ => {}
                                    }
                                }
                                proto_route.next_hops.push(next_hop);
                            }
                        }
                        _ => {}
                    }
                }
            } else {
                let mut next_hop = next_hop_default();
                for nla in route.nlas.iter() {
                    match nla {
                        Nla::Priority(p) => proto_route.priority = *p,
                        Nla::Gateway(g) => next_hop.gateway = parse_ipaddr(g)?.to_string(),
                        Nla::PrefSource(p) => next_hop.gateway = parse_ipaddr(p)?.to_string(),
                        Nla::Oif(i) => next_hop.interface = *i,
                        _ => {}
                    }
                }
                proto_route.next_hops.push(next_hop);
            }
            proto_routes.push(proto_route);
        }
        Ok(proto_routes)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_route(&self, 
        table: u32,
        ip_version: rtnetlink::IpVersion,
        destination: &str,
    ) -> Result<proto::sart::Route, Error> {
        tracing::info!("get route");
        let routes = self.list_routes(table, ip_version).await?;
        match routes.into_iter().find(|r| r.destination.eq(destination)) {
            Some(route) => Ok(route),
            None => Err(Error::DestinationNotFound)
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn add_route(&self, route: proto::sart::Route) -> Result<(), Error> {
        let rt = self.handler.route();
        let ip_version = ip_version_from(route.ip_version as u32)?;

        // look up existing routes
        let routes = self.list_routes(route.table_id, ip_version).await?;

        
        let mut req = rt.add().table(route.table_id as u8);
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn delete_route(&self, table: u8, ip_version: rtnetlink::IpVersion, destination: IpNet) -> Result<(), Error> {
        let rt = self.handler.route();

        // build rt_netlink message
        let mut msg = RouteMessage::default();
        msg.header.address_family = ip_version_into(&ip_version);
        msg.header.destination_prefix_length = destination.prefix_len();

        rt.del(msg).execute().await.unwrap();
        Ok(())
    }
}

pub(crate) enum AdministrativeDistance {
    Connected = 0,
    Static = 1,
    EBGP = 20,
    OSPF = 110,
    RIP = 120,
    IBGP = 200,
}

impl AdministrativeDistance {
    fn from_protocol(protocol: Protocol, internal: bool) -> AdministrativeDistance {
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
            _ => Err(Error::InvalidProtocol),
        }
    }
}

fn next_hop_default() -> proto::sart::RtNextHop {
    proto::sart::RtNextHop {
        gateway: String::new(),
        weight: 1,
        flags: 0,
        interface: 0,
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

fn parse_ipaddr(data: &Vec<u8>) -> Result<IpAddr, Error> {
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
