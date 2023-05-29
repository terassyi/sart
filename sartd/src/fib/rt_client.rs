use std::net::IpAddr;

use futures::TryStreamExt;
use ipnet::IpNet;
use netlink_packet_route::route::Nla;

use crate::fib::route::{
    ip_version_from, ip_version_into, AdministrativeDistance, NextHop, Protocol,
};

use super::{error::Error, route::Route};

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
        table: u8,
        ip_version: rtnetlink::IpVersion,
    ) -> Result<Vec<Route>, Error> {
        let routes = self.handler.route();
        let mut res = routes.get(ip_version.clone()).execute();

        tracing::info!("get routes");

        let mut routes = Vec::new();
        while let Some(msg) = res.try_next().await? {
            let route = Route::try_from(msg)?;
            routes.push(route);
        }
        Ok(routes)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_route(
        &self,
        table: u8,
        ip_version: rtnetlink::IpVersion,
        destination: &IpNet,
    ) -> Result<Route, Error> {
        tracing::info!("get route");
        let routes = self.list_routes(table, ip_version).await?;
        match routes.into_iter().find(|r| r.destination.eq(destination)) {
            Some(route) => Ok(route),
            None => Err(Error::DestinationNotFound),
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn add_route(&self, route: &Route, replace: bool) -> Result<(), Error> {
        tracing::info!("add a route");
        let rt = self.handler.route();
        let ip_version = route.version.clone();

        // look up existing routes
        let destination: IpNet = route.destination;

        let mut exist = false;
        let mut res = rt.get(route.version.clone()).execute();
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
            .kind(route.kind as u8)
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
        if let Some(src) = route.source {
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
                    // let mut next = netlinNextHop::default();
                    let mut next = netlink_packet_route::rtnl::route::nlas::NextHop::default();
                    next.flags =
                        netlink_packet_route::rtnl::route::nlas::NextHopFlags::from_bits_truncate(
                            h.flags.into(),
                        );
                    if h.weight > 0 {
                        next.hops = (h.weight - 1) as u8;
                    }
                    if h.interface != 0 {
                        next.interface_id = h.interface;
                    }

                    match h.gateway {
                        IpAddr::V4(addr) => next.nlas.push(Nla::Gateway(addr.octets().to_vec())),
                        IpAddr::V6(addr) => next.nlas.push(Nla::Gateway(addr.octets().to_vec())),
                    }
                    next
                })
                .collect();
            msg.nlas.push(Nla::MultiPath(n));
        } else if route.next_hops.is_empty() {
            // no next_hop
            return Err(Error::GatewayNotFound);
        }

        match route.next_hops[0].gateway {
            IpAddr::V4(addr) => msg.nlas.push(Nla::Gateway(addr.octets().to_vec())),
            IpAddr::V6(addr) => msg.nlas.push(Nla::Gateway(addr.octets().to_vec())),
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
        next_hops: Vec<NextHop>,
    ) -> Result<(), Error> {
        tracing::info!("add multiple path to the route");
        let mut route = self.get_route(table, ip_version, &destination).await?;
        for mut next_hop in next_hops.into_iter() {
            next_hop.weight = 1; // TODO: not to use fixed weight(=1)
            route.next_hops.push(next_hop);
        }

        self.add_route(&route, true).await
    }

    pub async fn delete_multi_path_route(
        &self,
        table: u8,
        ip_version: rtnetlink::IpVersion,
        destination: IpNet,
        gateways: Vec<IpAddr>,
    ) -> Result<(), Error> {
        tracing::info!("add multiple path to the route");
        let mut route = self.get_route(table, ip_version, &destination).await?;
        route
            .next_hops
            .retain(|n| gateways.iter().any(|g| n.gateway.ne(g)));

        self.add_route(&route, true).await
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        fib::route::{
            parse_ipaddr, AdministrativeDistance, Kind, NextHop, NextHopFlags, Protocol, Route,
            Scope,
        },
        proto,
    };
    use core::panic;
    use futures::TryStreamExt;
    use netlink_packet_route::route::Nla;
    use std::{
        net::{IpAddr, Ipv4Addr},
        os::fd::AsRawFd,
    };

    use super::RtClient;

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
                &Route {
                    table: 254,
                    version: rtnetlink::IpVersion::V4,
                    destination: "10.10.0.0/24".parse().unwrap(),
                    protocol: Protocol::Bgp,
                    scope: Scope::Universe,
                    kind: Kind::Unicast,
                    next_hops: vec![NextHop {
                        gateway: "10.0.0.1".parse().unwrap(),
                        weight: 20,
                        flags: NextHopFlags::Empty,
                        interface: 0,
                    }],
                    source: None,
                    ad: AdministrativeDistance::EBGP,
                    priority: 20,
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
                &Route {
                    table: 254,
                    version: rtnetlink::IpVersion::V4,
                    destination: "10.10.0.0/24".parse().unwrap(),
                    protocol: Protocol::Bgp,
                    scope: Scope::Universe,
                    kind: Kind::Unicast,
                    next_hops: vec![NextHop {
                        gateway: "10.0.1.1".parse().unwrap(),
                        weight: 20,
                        flags: NextHopFlags::Empty,
                        interface: 0,
                    }],
                    source: None,
                    ad: AdministrativeDistance::EBGP,
                    priority: 20,
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
                &Route {
                    table: 254,
                    version: rtnetlink::IpVersion::V4,
                    destination: "10.100.0.0/24".parse().unwrap(),
                    protocol: Protocol::Bgp,
                    scope: Scope::Universe,
                    kind: Kind::Unicast,
                    next_hops: vec![
                        NextHop {
                            gateway: "10.0.0.1".parse().unwrap(),
                            weight: 1,
                            flags: NextHopFlags::Empty,
                            interface: 0,
                        },
                        NextHop {
                            gateway: "10.0.1.1".parse().unwrap(),
                            weight: 1,
                            flags: NextHopFlags::Empty,
                            interface: 0,
                        },
                    ],
                    source: None,
                    ad: AdministrativeDistance::EBGP,
                    priority: 20,
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
                                    if !expected.iter().any(|a| a.eq(&gaddr)) {
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

        // delete multi path route
        ns_rt
            .delete_multi_path_route(
                254,
                rtnetlink::IpVersion::V4,
                "10.100.0.0/24".parse().unwrap(),
                vec!["10.0.0.1".parse().unwrap()],
            )
            .await
            .map_err(|e| format!("{e}"))
            .unwrap();

        let mut a = ns_rt
            .handler
            .route()
            .get(rtnetlink::IpVersion::V4)
            .execute();
        let mut existing = false;
        while let Some(route) = a.try_next().await.unwrap() {
            let (addr, prefix_len) = route.destination_prefix().unwrap();
            if addr.to_string() == "10.100.0.0" && prefix_len == 24 {
                for nla in route.nlas.iter() {
                    match nla {
                        Nla::MultiPath(_) => panic!("multi path is not expected"),
                        Nla::Gateway(g) => {
                            let a = parse_ipaddr(g).unwrap();
                            existing = a.eq(&"10.0.1.1".parse::<IpAddr>().unwrap());
                        }
                        _ => {}
                    }
                }
            }
        }
        if !existing {
            panic!("a route should have one gateway");
        }

        // add multi path route
        ns_rt
            .add_multi_path_route(
                254,
                rtnetlink::IpVersion::V4,
                "10.100.0.0/24".parse().unwrap(),
                vec![NextHop {
                    gateway: "10.0.0.1".parse().unwrap(),
                    weight: 1,
                    flags: NextHopFlags::Empty,
                    interface: 0,
                }],
            )
            .await
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
                                    if !expected.iter().any(|a| a.eq(&gaddr)) {
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
