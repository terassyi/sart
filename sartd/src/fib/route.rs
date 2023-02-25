use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use futures::TryStreamExt;
use ipnet::IpNet;
use netlink_packet_route::route::{NextHop, NextHopFlags, Nla};

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
                table: route.header.table as u32,
                version: route.header.address_family as i32,
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
        let ip_version = ip_version_from(route.version as u32)?;

        // look up existing routes
        let destination: IpNet = route
            .destination
            .parse()
            .map_err(|_| Error::FailedToParseAddress)?;

        let mut exist = false;
        let mut res = rt.get(ip_version.clone()).execute();
        while let Some(r) = res.try_next().await? {
            if let Some((dst, prefix_len)) = r.destination_prefix() {
                if destination.prefix_len() == prefix_len
                    && destination.addr().eq(&dst)
                    && r.header.table == route.table as u8
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
                        rt.del(r).execute().await?;
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
            .table(route.table as u8)
            .kind(route.r#type as u8)
            .protocol(route.protocol as u8)
            .scope(route.scope as u8);

        let msg = req.message_mut();
        msg.header.address_family = ip_version_into(&ip_version);

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
                            IpAddr::V4(addr) => {
                                next.nlas.push(Nla::Gateway(addr.octets().to_vec()))
                            }
                            IpAddr::V6(addr) => {
                                next.nlas.push(Nla::Gateway(addr.octets().to_vec()))
                            }
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

    #[tracing::instrument(skip(self))]
    pub async fn add_multi_path_route(
        &self,
        table: u8,
        ip_version: rtnetlink::IpVersion,
        destination: IpNet,
        next_hops: Vec<proto::sart::NextHop>,
    ) -> Result<(), Error> {
        tracing::info!("add multiple path to the route");
        if next_hops.is_empty() {
            return Ok(());
        }
        let rt = self.handler.route();

        let weight = next_hops[0].weight;
        let ip_version_num = ip_version_into(&ip_version);
        let mut res = rt.get(ip_version).execute();
        while let Some(mut route) = res.try_next().await? {
            if let Some((dst, prefix_len)) = route.destination_prefix() {
                if destination.prefix_len() == prefix_len
                    && destination.addr().eq(&dst)
                    && route.header.table == table
                {
                    // delete the old route
                    tracing::info!("delete the old route");
                    rt.del(route.clone()).execute().await?;
                    let mut nhs: Vec<NextHop> = next_hops
                        .iter()
                        .map(|n| {
                            let mut rt_n = NextHop::default();
                            rt_n.hops = (n.weight - 1) as u8;
                            rt_n.flags = NextHopFlags::from_bits_truncate(n.flags as u8);
                            let addr = n.gateway.parse::<IpAddr>().unwrap();
                            rt_n.nlas.push(Nla::Gateway(ipaddr_to_vec(addr)));
                            rt_n
                        })
                        .collect();
                    let mut new_multi_path = false;
                    for nla in route.nlas.iter_mut() {
                        if let Nla::Gateway(g) = nla {
                            // the old route must not be the multi_path route
                            let mut n = NextHop::default();
                            n.hops = (weight - 1) as u8;
                            n.flags = NextHopFlags::empty();
                            n.nlas.push(Nla::Gateway(g.to_vec()));
                            nhs.push(n);
                            // route must not have multi_path attribute
                            new_multi_path = true;
                        }
                        if let Nla::MultiPath(multi) = nla {
                            multi.append(&mut nhs);
                        }
                    }
                    route.nlas = route
                        .nlas
                        .into_iter()
                        .filter(|nla| match nla {
                            Nla::Gateway(_) | Nla::Oif(_) | Nla::Source(_) => false,
                            _ => true,
                        })
                        .collect();
                    if new_multi_path {
                        route.nlas.push(Nla::MultiPath(nhs));
                    }

                    // add as new route
                    let mut req = rt.add();
                    let mut msg = req.message_mut();
                    msg = &mut route;
                    tracing::info!(msg=?msg,"append multi path");
                    match ip_version_num {
                        2 => req.v4().execute().await?,
                        10 => req.v6().execute().await?,
                        _ => return Err(Error::InvalidProtocol),
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn delete_multi_path_route(
        &self,
        table: u8,
        ip_version: rtnetlink::IpVersion,
        destination: IpNet,
        gateways: Vec<IpAddr>,
    ) -> Result<(), Error> {
        tracing::info!("add multiple path to the route");
        if gateways.is_empty() {
            return Ok(());
        }
        let rt = self.handler.route();

        let ip_version_num = ip_version_into(&ip_version);
        let mut res = rt.get(ip_version).execute();
        while let Some(mut route) = res.try_next().await? {
            if let Some((dst, prefix_len)) = route.destination_prefix() {
                if destination.prefix_len() == prefix_len
                    && destination.addr().eq(&dst)
                    && route.header.table == table
                {
                    // delete the old route
                    tracing::info!("delete the old route");
                    rt.del(route.clone()).execute().await?;

                    match route.nlas.iter().find(|&nla| {
                        if let Nla::MultiPath(m) = nla {
                            true
                        } else {
                            false
                        }
                    }) {
                        Some(multi_path) => {
                            if let Nla::MultiPath(next_hops) = multi_path {
                                if next_hops.len() - gateways.len() > 1 {
                                    let a = next_hops
                                        .iter()
                                        .filter(|&n| match n.gateway() {
                                            Some(gateway) => {
                                                gateways.iter().find(|&g| gateway.eq(g)).is_none()
                                            }
                                            None => true,
                                        })
                                        .collect::<Vec<&NextHop>>();
                                } else {
                                    let gateway = match next_hops.iter().find(|&n| {
                                        gateways
                                            .iter()
                                            .find(|&g| g.eq(&n.gateway().unwrap()))
                                            .is_none()
                                    }) {
                                        Some(gateway) => gateway,
                                        None => return Err(Error::GatewayNotFound),
                                    };
                                }
                            }
                        }
                        None => {}
                    }

                    // add as new route
                    let mut req = rt.add();
                    let mut msg = req.message_mut();
                    msg = &mut route;
                    tracing::info!(msg=?msg,"drop multi path");
                    match ip_version_num {
                        2 => req.v4().execute().await?,
                        10 => req.v6().execute().await?,
                        _ => return Err(Error::InvalidProtocol),
                    }
                }
            }
        }

        Ok(())
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

fn next_hop_default() -> proto::sart::NextHop {
    proto::sart::NextHop {
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

fn ipaddr_to_vec(addr: IpAddr) -> Vec<u8> {
    match addr {
        IpAddr::V4(a) => a.octets().to_vec(),
        IpAddr::V6(a) => a.octets().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use core::panic;
    use futures::TryStreamExt;
    use netlink_packet_route::route::Nla;
    use rtnetlink::NetworkNamespace;
    use std::{
        net::{IpAddr, Ipv4Addr},
        os::fd::AsRawFd,
    };

    use crate::proto;

    use super::{parse_ipaddr, RtClient};

    #[tokio::test]
    async fn test_rt_client() {
        // prepare netns
        let ns = netns_rs::NetNs::new("test-rt-client").unwrap();
        let fd = ns.file().as_raw_fd();

        // prepare rtnetlink handler
        let (conn, handler, _rx) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);

        let rt = RtClient::new(handler);

        // prepare links
        let (v0, p0) = ("v0".to_string(), "p0".to_string());
        let (v1, p1) = ("v1".to_string(), "p1".to_string());
        rt.handler
            .link()
            .add()
            .veth(v0.clone(), p0)
            .execute()
            .await
            .unwrap();
        rt.handler
            .link()
            .add()
            .veth(v1.clone(), p1)
            .execute()
            .await
            .unwrap();

        let mut v0_idx = 0;
        let mut v1_idx = 0;
        let mut v0_l = rt.handler.link().get().match_name(v0.clone()).execute();
        while let Some(l) = v0_l.try_next().await.unwrap() {
            v0_idx = l.header.index;
        }
        let mut v1_l = rt.handler.link().get().match_name(v1.clone()).execute();
        while let Some(l) = v1_l.try_next().await.unwrap() {
            v1_idx = l.header.index;
        }

        // set ns
        rt.handler
            .link()
            .set(v0_idx)
            .setns_by_fd(fd)
            .execute()
            .await
            .unwrap();
        rt.handler
            .link()
            .set(v1_idx)
            .setns_by_fd(fd)
            .execute()
            .await
            .unwrap();

        ns.enter().unwrap();
        let (ns_conn, ns_handler, _rx) = rtnetlink::new_connection().unwrap();
        tokio::spawn(ns_conn);

        // set addr
        let mut v0_l = ns_handler.link().get().match_index(v0_idx).execute();
        while let Some(l) = v0_l.try_next().await.map_err(|e| format!("{e}")).unwrap() {
            v0_idx = l.header.index;
        }
        let mut v1_l = ns_handler.link().get().match_name(v1).execute();
        while let Some(l) = v1_l.try_next().await.map_err(|e| format!("{e}")).unwrap() {
            v1_idx = l.header.index;
        }
        ns_handler
            .address()
            .add(v0_idx, "10.0.0.1".parse().unwrap(), 24)
            .execute()
            .await
            .map_err(|e| format!("{e}"))
            .unwrap();
        ns_handler
            .address()
            .add(v1_idx, "10.0.1.1".parse().unwrap(), 24)
            .execute()
            .await
            .map_err(|e| format!("{e}"))
            .unwrap();

        // set up
        ns_handler.link().set(1).up().execute().await.unwrap();
        ns_handler.link().set(v0_idx).up().execute().await.unwrap();
        ns_handler.link().set(v1_idx).up().execute().await.unwrap();

        let ns_rt = RtClient::new(ns_handler);

        ns_rt
            .add_route(
                &proto::sart::Route {
                    table: 254,
                    version: 2,
                    destination: "10.10.0.0/24".to_string(),
                    protocol: 186,
                    scope: 0,
                    r#type: 1,
                    next_hops: vec![proto::sart::NextHop {
                        gateway: "10.0.0.1".to_string(),
                        weight: 20,
                        flags: 0,
                        interface: 0,
                    }],
                    source: String::new(),
                    ad: 20,
                    priority: 20,
                    ibgp: false,
                },
                false,
            )
            .await
            .unwrap();

        let mut a = ns_rt
            .handler
            .route()
            .get(rtnetlink::IpVersion::V4)
            .execute();
        let mut existing = false;
        while let Some(route) = a.try_next().await.unwrap() {
            let (addr, prefix_len) = route.destination_prefix().unwrap();
            if addr.to_string() == "10.10.0.0" && prefix_len == 24 {
                assert_eq!(
                    route.gateway().unwrap(),
                    "10.0.0.1".parse::<Ipv4Addr>().unwrap()
                );
                existing = true;
            }
        }
        if !existing {
            panic!("a route should exist");
        }

        // replace
        ns_rt
            .add_route(
                &proto::sart::Route {
                    table: 254,
                    version: 2,
                    destination: "10.10.0.0/24".to_string(),
                    protocol: 186,
                    scope: 0,
                    r#type: 1,
                    next_hops: vec![proto::sart::NextHop {
                        gateway: "10.0.1.1".to_string(),
                        weight: 20,
                        flags: 0,
                        interface: 0,
                    }],
                    source: String::new(),
                    ad: 20,
                    priority: 20,
                    ibgp: false,
                },
                true,
            )
            .await
            .map_err(|e| format!("{e}"))
            .unwrap();

        let mut a = ns_rt
            .handler
            .route()
            .get(rtnetlink::IpVersion::V4)
            .execute();
        existing = false;
        while let Some(route) = a.try_next().await.unwrap() {
            let (addr, prefix_len) = route.destination_prefix().unwrap();
            if addr.to_string() == "10.10.0.0" && prefix_len == 24 {
                assert_eq!(
                    route.gateway().unwrap(),
                    "10.0.1.1".parse::<Ipv4Addr>().unwrap()
                );
                existing = true;
            }
        }
        if !existing {
            panic!("a route should exist");
        }

        // delete route
        ns_rt
            .delete_route(
                254,
                rtnetlink::IpVersion::V4,
                "10.10.0.0/24".parse().unwrap(),
            )
            .await
            .unwrap();
        let mut a = ns_rt
            .handler
            .route()
            .get(rtnetlink::IpVersion::V4)
            .execute();
        existing = false;
        while let Some(route) = a.try_next().await.unwrap() {
            let (addr, prefix_len) = route.destination_prefix().unwrap();
            if addr.to_string() == "10.10.0.0" && prefix_len == 24 {
                assert_eq!(
                    route.gateway().unwrap(),
                    "10.0.1.1".parse::<Ipv4Addr>().unwrap()
                );
                existing = true;
            }
        }
        if existing {
            panic!("a route should not exist");
        }

        // add a route that has multiple path
        ns_rt
            .add_route(
                &proto::sart::Route {
                    table: 254,
                    version: 2,
                    destination: "10.100.0.0/24".to_string(),
                    protocol: 186,
                    scope: 0,
                    r#type: 1,
                    next_hops: vec![
                        proto::sart::NextHop {
                            gateway: "10.0.0.1".to_string(),
                            weight: 20,
                            flags: 0,
                            interface: 0,
                        },
                        proto::sart::NextHop {
                            gateway: "10.0.1.1".to_string(),
                            weight: 20,
                            flags: 0,
                            interface: 0,
                        },
                    ],
                    source: String::new(),
                    ad: 20,
                    priority: 20,
                    ibgp: false,
                },
                false,
            )
            .await
            .map_err(|e| format!("{e}"))
            .unwrap();

        let mut a = ns_rt
            .handler
            .route()
            .get(rtnetlink::IpVersion::V4)
            .execute();
        let mut existing = 0;
        let expected: Vec<IpAddr> = vec!["10.0.0.1".parse().unwrap(), "10.0.1.1".parse().unwrap()];
        while let Some(route) = a.try_next().await.unwrap() {
            let (addr, prefix_len) = route.destination_prefix().unwrap();
            if addr.to_string() == "10.100.0.0" && prefix_len == 24 {
                for nla in route.nlas.iter() {
                    if let Nla::MultiPath(multi) = nla {
                        for m in multi.iter() {
                            for n in m.nlas.iter() {
                                if let Nla::Gateway(g) = n {
                                    let gaddr = parse_ipaddr(g).unwrap();
                                    if expected.iter().find(|&a| a.eq(&gaddr)).is_none() {
                                        panic!("found unexpected gateway address")
                                    }
                                    existing += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        if existing != 2 {
            panic!("a route should have two gateways");
        }

        ns.remove().unwrap();
    }
}
