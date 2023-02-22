use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use futures::TryStreamExt;
use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use netlink_packet_route::{
    route::{NextHop, NextHopBuffer, NextHopFlags, Nla},
    RouteFlags, RouteHeader, RouteMessage,
};

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
            println!("{:?}", route);
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
                priority: 0,
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
                                next_hop.weight = h.hops as u32;
                                next_hop.interface = h.interface_id;
                                next_hop.flags = h.flags.bits() as i32;
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
    pub async fn get_route(
        &self,
        table: u32,
        ip_version: rtnetlink::IpVersion,
        destination: &str,
    ) -> Result<proto::sart::Route, Error> {
        tracing::info!("get route");
        let routes = self.list_routes(table, ip_version).await?;
        match routes.into_iter().find(|r| r.destination.eq(destination)) {
            Some(route) => Ok(route),
            None => Err(Error::DestinationNotFound),
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn add_route(&self, route: &proto::sart::Route, replace: bool) -> Result<(), Error> {
        tracing::info!("add a route");
        let rt = self.handler.route();
        let ip_version = ip_version_from(route.ip_version as u32)?;

        // look up existing routes
        let destination: IpNet = route
            .destination
            .parse()
            .map_err(|_| Error::FailedToParseAddress)?;

        let dest = match destination {
            IpNet::V4(a) => a,
            IpNet::V6(_) => panic!(),
        };

        let mut exist = false;
        let mut res = rt.get(ip_version.clone()).execute();
        while let Some(r) = res.try_next().await? {
            if let Some((dst, prefix_len)) = r.destination_prefix() {
                if destination.prefix_len() == prefix_len
                    && destination.addr().eq(&dst)
                    && r.header.table == route.table_id as u8
                {
                    // compare priority by ad value
                    tracing::info!("an existing route is found");
                    // delete the old route
                    let existing_ad = AdministrativeDistance::from_protocol(
                        Protocol::try_from(r.header.protocol)?,
                        false,
                    );
                    let new_ad = AdministrativeDistance::try_from(route.ad as u8)?;
                    if existing_ad >= new_ad {
                        tracing::info!(existing_ad=?existing_ad,new_ad=?new_ad, "replace it");
                        // rt.del(r).execute().await?;
                        exist = true;
                    } else {
                        tracing::info!("the existing route has lower ad value. nothing to do.");
                        return Ok(());
                    }
                }
            }
        }

        if !replace && exist {
            return Err(Error::AlreadyExists);
        }
        let mut req = rt
            .add()
            .table(route.table_id as u8)
            .kind(route.r#type as u8)
            .protocol(route.protocol as u8)
            .scope(route.scope as u8);

        if replace {
            req = req.replace();
        }

        let msg = req.message_mut();

        // destination
        msg.header.destination_prefix_length = destination.prefix_len();
        match destination.addr() {
            IpAddr::V4(a) => msg.nlas.push(Nla::Destination(a.octets().to_vec())),
            IpAddr::V6(a) => msg.nlas.push(Nla::Destination(a.octets().to_vec())),
        }

        // source
        if let Ok(src) = route.source.parse::<IpAddr>() {
            match src {
                IpAddr::V4(addr) => msg.nlas.push(Nla::Source(addr.octets().to_vec())),
                IpAddr::V6(addr) => msg.nlas.push(Nla::Source(addr.octets().to_vec())),
            }
        }

        if route.priority != 0 {
            msg.nlas.push(Nla::Priority(route.priority));
        }
        // next hop
        if route.next_hops.len() > 1 {
            // multi_path
            let n = route
                .next_hops
                .iter()
                .map(|h| {
                    let mut next = NextHop::default();
                    next.flags = NextHopFlags::from_bits_truncate(h.flags as u8);
                    if h.weight > 0 {
                        next.hops = (h.weight - 1) as u8;
                    }
                    if h.interface != 0 {
                        next.interface_id = h.interface;
                    }
                    if let Ok(gateway) = h.gateway.parse::<IpAddr>() {
                        match gateway {
                            IpAddr::V4(addr) => msg.nlas.push(Nla::Gateway(addr.octets().to_vec())),
                            IpAddr::V6(addr) => msg.nlas.push(Nla::Gateway(addr.octets().to_vec())),
                        }
                    }
                    next
                })
                .collect();
            msg.nlas.push(Nla::MultiPath(n));
        } else if route.next_hops.is_empty() {
            // no next_hop
            return Err(Error::GatewayNotFound);
        } else if let Ok(gateway) = route.next_hops[0].gateway.parse::<IpAddr>() {
            match gateway {
                IpAddr::V4(addr) => msg.nlas.push(Nla::Gateway(addr.octets().to_vec())),
                IpAddr::V6(addr) => msg.nlas.push(Nla::Gateway(addr.octets().to_vec())),
            }
        }

        match ip_version {
            rtnetlink::IpVersion::V4 => req.v4().execute().await?,
            rtnetlink::IpVersion::V6 => req.v6().execute().await?,
        }
        // req.execute().await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn delete_route(
        &self,
        table: u8,
        ip_version: rtnetlink::IpVersion,
        destination: IpNet,
    ) -> Result<(), Error> {
        tracing::info!("delete a route");
        let rt = self.handler.route();

        let mut res = rt.get(ip_version).execute();
        while let Some(route) = res.try_next().await? {
            if let Some((dst, prefix_len)) = route.destination_prefix() {
                if destination.prefix_len() == prefix_len
                    && destination.addr().eq(&dst)
                    && route.header.table == table
                {
                    rt.del(route).execute().await?;
                    return Ok(());
                }
            }
        }
        Err(Error::DestinationNotFound)
    }
}

#[derive(Debug, PartialEq, PartialOrd, Eq, Ord)]
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

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unspec => write!(f, "Unspec"),
            Self::Redirect => write!(f, "Redirect"),
            Self::Kernel => write!(f, "Kernel"),
            Self::Boot => write!(f, "Boot"),
            Self::Static => write!(f, "Static"),
            Self::Bgp => write!(f, "Bgp"),
            Self::Ospf => write!(f, "Ospf"),
            Self::Rip => write!(f, "Rip"),
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

fn next_hop_default() -> proto::sart::RtNextHop {
    proto::sart::RtNextHop {
        gateway: String::new(),
        weight: 0,
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

#[cfg(test)]
mod tests {
    use futures::TryStreamExt;
    use rtnetlink::NetworkNamespace;

    use super::RtClient;

    #[tokio::test]
    async fn test_rtnetlink() {}
}
