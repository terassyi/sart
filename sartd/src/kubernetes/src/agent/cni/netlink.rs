use std::{collections::HashMap, net::IpAddr, str::FromStr};

use futures::TryStreamExt;

use ipnet::IpNet;
use netlink_packet_route::{
    link::LinkAttribute,
    route::{RouteAddress, RouteAttribute, RouteProtocol, RouteScope},
    rule::RuleAction,
};

use rtnetlink::{Handle, IpVersion};
use thiserror::Error;

use super::netns::NetNS;

const ROUTE_SCOPE_SART: u8 = 50;
pub const DEFAULT_ROUTE_IPV4: &str = "0.0.0.0/0";
pub const DEFAULT_ROUTE_IPV6: &str = "::/0";

#[derive(Debug, Error)]
pub enum Error {
    #[error("Open netlink socket: {0}")]
    Open(#[source] std::io::Error),

    #[error("Netlink: {0}")]
    Netlink(#[source] rtnetlink::Error),

    #[error("Create veth: {0}")]
    Veth(#[source] rtnetlink::Error),

    #[error("Set NetNS: {0}")]
    NetNS(#[source] rtnetlink::Error),

    #[error("Link up: {0}")]
    LinkUp(#[source] rtnetlink::Error),

    #[error("Link: {0}")]
    Link(#[source] rtnetlink::Error),

    #[error("Address: {0}")]
    Address(#[source] rtnetlink::Error),

    #[error("Route: {0}")]
    Route(#[source] rtnetlink::Error),

    #[error("Rule: {0}")]
    Rule(#[source] rtnetlink::Error),

    #[error("Link not found: {0}")]
    LinkNotFound(String),

    #[error("Invalid container-id")]
    InvalidContainerId,

    #[error("Invalid addrss: {0}")]
    InvalidAddress(String),

    #[error("Route not found: {0}")]
    RouteNotFound(String),

    #[error("Invalid Mac address")]
    InvalidMacAddress,

    #[error("Invalid link alias: {0}")]
    InvalidLinkAlias(String),
}

pub async fn add_veth_pair(container_id: &str, ifname: &str) -> Result<String, Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    if container_id.len() < 8 {
        return Err(Error::InvalidContainerId);
    }

    let short_id = &container_id[..8];
    let host_side = format!("veth{short_id}");

    handle
        .link()
        .add()
        .veth(host_side.clone(), ifname.to_string())
        .execute()
        .await
        .map_err(Error::Veth)?;
    Ok(host_side)
}

pub async fn move_netns(name: &str, netns: &NetNS) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let index = get_link_index_by_name(&handle, name).await?;
    handle
        .link()
        .set(index)
        .setns_by_fd(netns.fd())
        .execute()
        .await
        .map_err(Error::NetNS)?;
    Ok(())
}

async fn del_veth(name: &str) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let index = get_link_index_by_name(&handle, name).await?;
    handle
        .link()
        .del(index)
        .execute()
        .await
        .map_err(Error::Veth)?;
    Ok(())
}

pub async fn link_up(name: &str) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let index = get_link_index_by_name(&handle, name).await?;
    handle
        .link()
        .set(index)
        .up()
        .execute()
        .await
        .map_err(Error::LinkUp)?;
    Ok(())
}

pub async fn get_link_mac(name: &str) -> Result<String, Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let mut links = handle.link().get().match_name(name.to_string()).execute();
    if let Some(msg) = links.try_next().await.map_err(Error::Link)? {
        for attr in msg.attributes.into_iter() {
            if let LinkAttribute::Address(v) = attr {
                if v.len() != 6 {
                    return Err(Error::InvalidMacAddress);
                }
                return Ok(format!(
                    "{:#02x}:{:#02x}:{:#02x}:{:#02x}:{:#02x}:{:#02x}",
                    v[0], v[1], v[2], v[3], v[4], v[5]
                ));
            }
        }
    }
    Err(Error::LinkNotFound(name.to_string()))
}

pub async fn set_alias(name: &str, alias: &str) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let index = get_link_index_by_name(&handle, name).await?;
    let mut req = handle.link().set(index);
    let msg = req.message_mut();
    msg.attributes
        .push(LinkAttribute::IfAlias(alias.to_string()));
    req.execute().await.map_err(Error::Link)
}

pub async fn add_addr(name: &str, addr: &IpNet) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let index = get_link_index_by_name(&handle, name).await?;

    handle
        .address()
        .add(index, addr.addr(), addr.prefix_len())
        .execute()
        .await
        .map_err(Error::Address)
}

pub async fn add_route(
    dst: &IpNet,
    gateway: Option<IpAddr>,
    device: &str,
    scope: RouteScope,
) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let index = get_link_index_by_name(&handle, device).await?;

    let req = handle
        .route()
        .add()
        .scope(scope)
        .output_interface(index)
        .protocol(RouteProtocol::Other(ROUTE_SCOPE_SART));

    match dst {
        IpNet::V4(dst) => {
            let req = req.v4().destination_prefix(dst.addr(), dst.prefix_len());
            if let Some(IpAddr::V4(gw)) = gateway {
                req.gateway(gw).execute().await.map_err(Error::Route)?;
            } else {
                req.execute().await.map_err(Error::Route)?;
            }
        }
        IpNet::V6(dst) => {
            let req = req.v6().destination_prefix(dst.addr(), dst.prefix_len());
            if let Some(IpAddr::V6(gw)) = gateway {
                req.gateway(gw).execute().await.map_err(Error::Route)?;
            } else {
                req.execute().await.map_err(Error::Route)?;
            }
        }
    };

    Ok(())
}

pub async fn add_rule(table: u32, protocol: IpAddr) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let req = handle
        .rule()
        .add()
        .table_id(table)
        .action(RuleAction::ToTable)
        .priority(2000);

    match protocol {
        IpAddr::V4(_) => req.v4().execute().await.map_err(Error::Rule),
        IpAddr::V6(_) => req.v6().execute().await.map_err(Error::Rule),
    }
}

pub async fn add_route_in_table(
    dst: &IpNet,
    gateway: Option<IpAddr>,
    device: &str,
    scope: RouteScope,
    table: u32,
) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let index = get_link_index_by_name(&handle, device).await?;

    let req = handle
        .route()
        .add()
        .scope(scope)
        .output_interface(index)
        .table_id(table)
        .protocol(RouteProtocol::Other(ROUTE_SCOPE_SART));
    match dst {
        IpNet::V4(dst) => {
            if let Some(IpAddr::V4(gw)) = gateway {
                req.v4()
                    .gateway(gw)
                    .destination_prefix(dst.addr(), dst.prefix_len())
                    .execute()
                    .await
                    .map_err(Error::Route)?;
            } else {
                req.v4()
                    .destination_prefix(dst.addr(), dst.prefix_len())
                    .execute()
                    .await
                    .map_err(Error::Route)?;
            }
        }
        IpNet::V6(dst) => {
            if let Some(IpAddr::V6(gw)) = gateway {
                req.v6()
                    .gateway(gw)
                    .destination_prefix(dst.addr(), dst.prefix_len())
                    .execute()
                    .await
                    .map_err(Error::Route)?;
            } else {
                req.v6()
                    .destination_prefix(dst.addr(), dst.prefix_len())
                    .execute()
                    .await
                    .map_err(Error::Route)?;
            }
        }
    }
    Ok(())
}

pub async fn del_link(name: &str) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let index = get_link_index_by_name(&handle, name).await?;

    handle
        .link()
        .del(index)
        .execute()
        .await
        .map_err(Error::Link)?;
    Ok(())
}

pub async fn del_route(dst: &IpNet) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let ip_version = match dst {
        IpNet::V4(_) => IpVersion::V4,
        IpNet::V6(_) => IpVersion::V6,
    };
    let mut res = handle.route().get(ip_version.clone()).execute();

    let default_route = match dst {
        IpNet::V4(_) => IpNet::from_str(DEFAULT_ROUTE_IPV4).unwrap(),
        IpNet::V6(_) => IpNet::from_str(DEFAULT_ROUTE_IPV6).unwrap(),
    };

    while let Some(r) = res.try_next().await.map_err(Error::Route)? {
        let dst_prefix_len = r.header.destination_prefix_length;
        let mut is_default = true;
        for attr in r.attributes.iter() {
            match ip_version {
                IpVersion::V4 => {
                    if let RouteAttribute::Destination(RouteAddress::Inet(addr)) = attr {
                        is_default = false;
                        if addr.eq(&dst.addr()) && dst_prefix_len == dst.prefix_len() {
                            // delete it
                            return handle.route().del(r).execute().await.map_err(Error::Route);
                        }
                    }
                }
                IpVersion::V6 => {
                    if let RouteAttribute::Destination(RouteAddress::Inet6(addr)) = attr {
                        is_default = false;
                        if addr.eq(&dst.addr()) && dst_prefix_len == dst.prefix_len() {
                            // delete it
                            return handle.route().del(r).execute().await.map_err(Error::Route);
                        }
                    }
                }
            }
        }
        if is_default && default_route.eq(dst) {
            return handle.route().del(r).execute().await.map_err(Error::Route);
        }
    }
    Err(Error::RouteNotFound(dst.to_string()))
}

pub async fn del_route_in_table(dst: &IpNet, table: u32) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let ip_version = match dst {
        IpNet::V4(_) => IpVersion::V4,
        IpNet::V6(_) => IpVersion::V6,
    };
    let mut res = handle.route().get(ip_version.clone()).execute();

    let default_route = match dst {
        IpNet::V4(_) => IpNet::from_str(DEFAULT_ROUTE_IPV4).unwrap(),
        IpNet::V6(_) => IpNet::from_str(DEFAULT_ROUTE_IPV6).unwrap(),
    };

    while let Some(r) = res.try_next().await.map_err(Error::Route)? {
        let dst_prefix_len = r.header.destination_prefix_length;
        let mut is_default = true;
        let mut is_target_table = false;
        for attr in r.attributes.iter() {
            if let RouteAttribute::Table(id) = attr {
                if table.ne(id) {
                    continue;
                }
            }
            is_target_table = true;
            match ip_version {
                IpVersion::V4 => {
                    if let RouteAttribute::Destination(RouteAddress::Inet(addr)) = attr {
                        is_default = false;
                        if addr.eq(&dst.addr()) && dst_prefix_len == dst.prefix_len() {
                            // delete it
                            return handle.route().del(r).execute().await.map_err(Error::Route);
                        }
                    }
                }
                IpVersion::V6 => {
                    if let RouteAttribute::Destination(RouteAddress::Inet6(addr)) = attr {
                        is_default = false;
                        if addr.eq(&dst.addr()) && dst_prefix_len == dst.prefix_len() {
                            // delete it
                            return handle.route().del(r).execute().await.map_err(Error::Route);
                        }
                    }
                }
            }
        }
        if is_target_table && is_default && default_route.eq(dst) {
            return handle.route().del(r).execute().await.map_err(Error::Route);
        }
    }
    Err(Error::RouteNotFound(dst.to_string()))
}

pub async fn del_rule(table: u32, protocol: &IpAddr) -> Result<(), Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);
    let ip_version = match protocol {
        IpAddr::V4(_) => IpVersion::V4,
        IpAddr::V6(_) => IpVersion::V6,
    };

    let mut rules = handle.rule().get(ip_version).execute();
    while let Some(r) = rules.try_next().await.map_err(Error::Rule)? {
        if r.header.table == table as u8 {
            return handle.rule().del(r).execute().await.map_err(Error::Rule);
        }
    }

    Ok(())
}

pub async fn get_rule(table: u32, protocol: &IpAddr) -> Result<Option<()>, Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);
    let ip_version = match protocol {
        IpAddr::V4(_) => IpVersion::V4,
        IpAddr::V6(_) => IpVersion::V6,
    };
    let mut rules = handle.rule().get(ip_version).execute();
    while let Some(r) = rules.try_next().await.map_err(Error::Rule)? {
        if r.header.table == table as u8 {
            return Ok(Some(()));
        }
    }
    Ok(None)
}

pub async fn get_link_index_by_name(handle: &Handle, name: &str) -> Result<u32, Error> {
    let mut links = handle.link().get().match_name(name.to_string()).execute();
    if let Some(msg) = links.try_next().await.map_err(Error::Link)? {
        return Ok(msg.header.index);
    }
    Err(Error::LinkNotFound(name.to_string()))
}

async fn get_link_name_by_index(handle: &Handle, index: u32) -> Result<String, Error> {
    let mut links = handle.link().get().match_index(index).execute();
    while let Some(l) = links.try_next().await.map_err(Error::Link)? {
        for attr in l.attributes.into_iter() {
            if let LinkAttribute::IfName(name) = attr {
                return Ok(name);
            }
        }
    }
    Err(Error::LinkNotFound(format!("{index}")))
}

/// This function gets ifindex from container_id.
/// To get it from container_id, container_id must be set as alias in advance.
pub async fn get_link_by_container_id(handle: &Handle, container_id: &str) -> Result<u32, Error> {
    let mut links = handle.link().get().execute();
    while let Some(l) = links.try_next().await.map_err(Error::Link)? {
        for attr in l.attributes.into_iter() {
            if let LinkAttribute::IfAlias(alias) = attr {
                if let Ok(link_info) = ContainerLinkInfo::from_str(&alias) {
                    if container_id.eq(&link_info.id) {
                        return Ok(l.header.index);
                    }
                }
            }
        }
    }
    Err(Error::LinkNotFound(container_id.to_string()))
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerLinkInfo {
    pub id: String,
    pub ifname: String,
    pub pool: String,
}

impl ContainerLinkInfo {
    pub fn new(id: &str, ifname: &str, pool: &str) -> ContainerLinkInfo {
        ContainerLinkInfo {
            id: id.to_string(),
            ifname: ifname.to_string(),
            pool: pool.to_string(),
        }
    }

    pub fn to_alias(&self) -> String {
        format!("{}/{}/{}", self.pool, self.id, self.ifname)
    }
}

impl FromStr for ContainerLinkInfo {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let alias_list = s.split('/').collect::<Vec<&str>>();
        if alias_list.len() != 3 {
            return Err(Error::InvalidLinkAlias(s.to_string()));
        }
        Ok(ContainerLinkInfo {
            pool: alias_list[0].to_string(),
            id: alias_list[1].to_string(),
            ifname: alias_list[2].to_string(),
        })
    }
}

// link_list returns key-value map of the pair of host side interface name and link alias.
pub async fn link_list() -> Result<HashMap<String, ContainerLinkInfo>, Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let mut containers = HashMap::new();

    let mut links = handle.link().get().execute();
    while let Some(l) = links.try_next().await.map_err(Error::Link)? {
        let mut ifname: Option<String> = None;
        let mut link_info: Option<ContainerLinkInfo> = None;
        for attr in l.attributes.into_iter() {
            if let LinkAttribute::IfAlias(ref alias) = attr {
                if let Ok(info) = ContainerLinkInfo::from_str(alias) {
                    link_info = Some(info);
                }
            }
            if let LinkAttribute::IfName(name) = attr {
                ifname = Some(name);
            }
        }
        if ifname.is_some() && link_info.is_some() {
            containers.insert(ifname.unwrap(), link_info.unwrap());
        }
    }
    Ok(containers)
}

pub async fn route_list(protocol: IpAddr, table_id: u32) -> Result<HashMap<String, IpAddr>, Error> {
    let (conn, handle, _) = rtnetlink::new_connection().map_err(Error::Open)?;
    tokio::spawn(conn);

    let mut route_map = HashMap::new();

    let ip_version = match protocol {
        IpAddr::V4(_) => IpVersion::V4,
        IpAddr::V6(_) => IpVersion::V6,
    };
    let mut res = handle.route().get(ip_version.clone()).execute();

    while let Some(r) = res.try_next().await.map_err(Error::Route)? {
        if r.header.table != table_id as u8 {
            continue;
        }
        let mut ifname: Option<String> = None;
        let mut addr: Option<IpAddr> = None;
        for attr in r.attributes.iter() {
            if let RouteAttribute::Oif(n) = attr {
                ifname = Some(get_link_name_by_index(&handle, *n).await?);
            }
            match ip_version {
                IpVersion::V4 => {
                    if let RouteAttribute::Destination(RouteAddress::Inet(a)) = attr {
                        addr = Some(IpAddr::V4(*a));
                    }
                }
                IpVersion::V6 => {
                    if let RouteAttribute::Destination(RouteAddress::Inet6(a)) = attr {
                        addr = Some(IpAddr::V6(*a));
                    }
                }
            }
        }
        if ifname.is_some() && addr.is_some() {
            route_map.insert(ifname.unwrap(), addr.unwrap());
        }
    }

    Ok(route_map)
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use netlink_packet_route::{address::AddressAttribute, link::LinkFlag};

    use crate::agent::cni::{netns, server::CNI_ROUTE_TABLE_ID};

    use super::*;

    struct TestNetNS {
        name: String,
        ns: NetNS,
        host_ns: NetNS,
    }

    impl TestNetNS {
        fn new() -> Self {
            use rand::Rng;

            let host_ns = netns::get_current_netns().unwrap();
            const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
            let mut rng = rand::thread_rng();

            let ns_prefix: String = (0..8)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect();

            let ns_name = format!("test-{ns_prefix}");
            let mut cmd = std::process::Command::new("ip");

            cmd.args(["netns", "add", &ns_name]);
            cmd.output().unwrap();
            let ns = NetNS::try_from(format!("/var/run/netns/{}", ns_name).as_str()).unwrap();
            TestNetNS {
                name: ns_name,
                ns,
                host_ns,
            }
        }

        fn enter(&self) {
            self.ns.enter().unwrap();
        }

        fn return_host_ns(&self) {
            self.host_ns.enter().unwrap();
        }

        fn cleanup(&self) {
            let mut cmd = std::process::Command::new("ip");
            cmd.args(["netns", "del", &self.name]);
            cmd.output().unwrap();
        }
    }

    impl Drop for TestNetNS {
        fn drop(&mut self) {
            self.cleanup()
        }
    }

    #[tokio::test]
    async fn test_add_veth_pair() {
        let test_ns = TestNetNS::new();

        test_ns.enter();

        let host_side = add_veth_pair("000011112222", "c-test").await.unwrap();
        assert_eq!("veth00001111", host_side.as_str());

        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);

        let _host_ifindex = get_link_index_by_name(&handle, &host_side).await.unwrap();
        let _container_ifindex = get_link_index_by_name(&handle, "c-test").await.unwrap();
    }

    #[tokio::test]
    async fn test_del_link() {
        let test_ns = TestNetNS::new();

        test_ns.enter();

        let host_side = add_veth_pair("000011112222", "c-test").await.unwrap();
        assert_eq!("veth00001111", host_side.as_str());

        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);

        let _host_ifindex = get_link_index_by_name(&handle, &host_side).await.unwrap();
        let _container_ifindex = get_link_index_by_name(&handle, "c-test").await.unwrap();

        del_link(&host_side).await.unwrap();
        let err = get_link_index_by_name(&handle, &host_side).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn test_move_netns() {
        let test_ns = TestNetNS::new();

        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);
        let host_side = add_veth_pair("000011112222", "c-test").await.unwrap();

        move_netns("c-test", &test_ns.ns).await.unwrap();

        let _exist = get_link_index_by_name(&handle, &host_side).await.unwrap();
        let err = get_link_index_by_name(&handle, "c-test").await;
        assert!(err.is_err()); // This error should be LinkNotFound

        test_ns.enter();
        let _exist = get_link_index_by_name(&handle, "c-test").await;
    }

    #[tokio::test]
    async fn test_link_up() {
        let test_ns = TestNetNS::new();

        test_ns.enter();

        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);
        let host_side = add_veth_pair("000011112222", "c-test").await.unwrap();

        link_up(&host_side).await.unwrap();

        let mut links = handle.link().get().match_name(host_side).execute();
        let mut link_up = false;
        match links.try_next().await.unwrap() {
            Some(msg) => {
                for flag in msg.header.flags.into_iter() {
                    if flag.eq(&LinkFlag::Up) {
                        link_up = true;
                    }
                }
            }
            None => panic!("Link not found"),
        }
        assert!(link_up);
    }

    #[tokio::test]
    async fn test_link_set_alias() {
        let test_ns = TestNetNS::new();

        test_ns.enter();
        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);
        let host_side = add_veth_pair("000011112222", "c-test").await.unwrap();

        set_alias(&host_side, "test-alias").await.unwrap();

        let mut links = handle.link().get().match_name(host_side).execute();
        let mut alias_set = false;
        match links.try_next().await.unwrap() {
            Some(msg) => {
                for attr in msg.attributes.into_iter() {
                    if let LinkAttribute::IfAlias(alias) = attr {
                        alias_set = true;
                        assert_eq!("test-alias", alias);
                    }
                }
            }
            None => panic!("Link not found"),
        }
        assert!(alias_set);
    }

    #[tokio::test]
    async fn test_add_addr() {
        let test_ns = TestNetNS::new();

        test_ns.enter();
        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);
        let host_side = add_veth_pair("000011112222", "c-test").await.unwrap();

        let test_addr = IpNet::from_str("10.0.0.1/24").unwrap();
        add_addr(&host_side, &test_addr).await.unwrap();

        let mut addrs = handle.address().get().execute();
        let mut addr_added = false;
        match addrs.try_next().await.unwrap() {
            Some(msg) => {
                for attr in msg.attributes.into_iter() {
                    if let AddressAttribute::Address(addr) = attr {
                        addr_added = true;
                        assert_eq!(addr, test_addr.addr());
                    }
                }
            }
            None => panic!("Address not found"),
        }
        assert!(addr_added);
    }

    #[tokio::test]
    async fn test_add_route() {
        let test_ns = TestNetNS::new();

        test_ns.enter();
        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);
        let host_side = add_veth_pair("000011112222", "c-test").await.unwrap();

        let test_addr = IpNet::from_str("10.0.0.1/24").unwrap();
        add_addr(&host_side, &test_addr).await.unwrap();
        link_up(&host_side).await.unwrap();

        let dst = IpNet::from_str("0.0.0.0/0").unwrap();
        let gateway = Some(test_addr.addr());

        add_route(&dst, gateway, &host_side, RouteScope::Universe)
            .await
            .unwrap();

        let mut routes = handle.route().get(IpVersion::V4).execute();
        let mut route_added = false;
        match routes.try_next().await.unwrap() {
            Some(msg) => {
                for attr in msg.attributes.into_iter() {
                    if let RouteAttribute::Gateway(RouteAddress::Inet(gw)) = attr {
                        route_added = true;
                        assert_eq!(test_addr.addr(), IpAddr::V4(gw));
                    }
                }
            }
            None => panic!("Route not found"),
        }
        assert!(route_added);
    }

    #[tokio::test]
    async fn test_add_route_in_table() {
        let test_ns = TestNetNS::new();

        test_ns.enter();
        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);
        let host_side = add_veth_pair("000011112222", "c-test").await.unwrap();

        let test_addr = IpNet::from_str("10.0.0.1/24").unwrap();
        add_addr(&host_side, &test_addr).await.unwrap();
        link_up(&host_side).await.unwrap();

        let dst = IpNet::from_str("0.0.0.0/0").unwrap();
        let gateway = Some(test_addr.addr());

        add_route_in_table(&dst, gateway, &host_side, RouteScope::Universe, 160)
            .await
            .unwrap();

        let mut routes = handle.route().get(IpVersion::V4).execute();
        let mut route_added = false;
        match routes.try_next().await.unwrap() {
            Some(msg) => {
                for attr in msg.attributes.into_iter() {
                    if let RouteAttribute::Gateway(RouteAddress::Inet(gw)) = attr {
                        route_added = true;
                        assert_eq!(test_addr.addr(), IpAddr::V4(gw));
                    }
                    if let RouteAttribute::Table(table_id) = attr {
                        assert_eq!(table_id, 160);
                    }
                }
            }
            None => panic!("Route not found"),
        }
        assert!(route_added);
    }

    #[tokio::test]
    async fn test_del_route() {
        let test_ns = TestNetNS::new();

        test_ns.enter();
        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);
        let host_side = add_veth_pair("000011112222", "c-test").await.unwrap();

        let test_addr = IpNet::from_str("10.0.0.1/24").unwrap();
        add_addr(&host_side, &test_addr).await.unwrap();
        link_up(&host_side).await.unwrap();

        let dst = IpNet::from_str("0.0.0.0/0").unwrap();
        let gateway = Some(test_addr.addr());

        add_route(&dst, gateway, &host_side, RouteScope::Universe)
            .await
            .unwrap();

        let mut routes = handle.route().get(IpVersion::V4).execute();
        let mut route_added = false;
        match routes.try_next().await.unwrap() {
            Some(msg) => {
                for attr in msg.attributes.into_iter() {
                    if let RouteAttribute::Gateway(RouteAddress::Inet(gw)) = attr {
                        route_added = true;
                        assert_eq!(test_addr.addr(), IpAddr::V4(gw));
                    }
                }
            }
            None => panic!("Route not found"),
        }
        assert!(route_added);

        del_route(&dst).await.unwrap();
        let mut routes = handle.route().get(IpVersion::V4).execute();
        let mut route_added = false;
        match routes.try_next().await.unwrap() {
            Some(msg) => {
                for attr in msg.attributes.into_iter() {
                    if let RouteAttribute::Gateway(RouteAddress::Inet(gw)) = attr {
                        route_added = true;
                        assert_eq!(test_addr.addr(), IpAddr::V4(gw));
                    }
                }
            }
            None => panic!("Route not found"),
        }
        assert!(!route_added);
    }

    #[tokio::test]
    async fn test_del_route_in_table() {
        let test_ns = TestNetNS::new();

        test_ns.enter();
        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);
        let host_side = add_veth_pair("000011112222", "c-test").await.unwrap();

        let test_addr = IpNet::from_str("10.0.0.1/24").unwrap();
        add_addr(&host_side, &test_addr).await.unwrap();
        link_up(&host_side).await.unwrap();

        let dst = IpNet::from_str("0.0.0.0/0").unwrap();
        let gateway = Some(test_addr.addr());

        add_route_in_table(&dst, gateway, &host_side, RouteScope::Universe, 160)
            .await
            .unwrap();

        let mut routes = handle.route().get(IpVersion::V4).execute();
        let mut route_added = false;
        match routes.try_next().await.unwrap() {
            Some(msg) => {
                for attr in msg.attributes.into_iter() {
                    if let RouteAttribute::Gateway(RouteAddress::Inet(gw)) = attr {
                        route_added = true;
                        assert_eq!(test_addr.addr(), IpAddr::V4(gw));
                    }
                    if let RouteAttribute::Table(table_id) = attr {
                        assert_eq!(table_id, 160);
                    }
                }
            }
            None => panic!("Route not found"),
        }
        assert!(route_added);

        del_route_in_table(&dst, 160).await.unwrap();
        let mut routes = handle.route().get(IpVersion::V4).execute();
        let mut route_added = false;
        match routes.try_next().await.unwrap() {
            Some(msg) => {
                for attr in msg.attributes.into_iter() {
                    if let RouteAttribute::Gateway(RouteAddress::Inet(gw)) = attr {
                        route_added = true;
                        assert_eq!(test_addr.addr(), IpAddr::V4(gw));
                    }
                }
            }
            None => panic!("Route not found"),
        }
        assert!(!route_added);
    }

    #[tokio::test]
    async fn test_link_list() {
        let test_ns = TestNetNS::new();

        test_ns.enter();
        let cont1 = ContainerLinkInfo::new("000011112222", "c0", "default");
        let cont2 = ContainerLinkInfo::new("333344445555", "c1", "another");
        let host_side1 = add_veth_pair(&cont1.id, &cont1.ifname).await.unwrap();
        let host_side2 = add_veth_pair(&cont2.id, &cont2.ifname).await.unwrap();

        set_alias(&host_side1, &cont1.to_alias()).await.unwrap();
        set_alias(&host_side2, &cont2.to_alias()).await.unwrap();

        let link_list = link_list().await.unwrap();

        assert_eq!(link_list.len(), 2);
        let res_cont1 = link_list.get(&host_side1).unwrap();
        let res_cont2 = link_list.get(&host_side2).unwrap();

        assert_eq!(&cont1, res_cont1);
        assert_eq!(&cont2, res_cont2);
    }

    #[tokio::test]
    async fn test_route_list() {
        let test_ns = TestNetNS::new();

        test_ns.enter();
        let host_side1 = add_veth_pair("000011112222", "c0").await.unwrap();

        let test_addr1 = IpNet::from_str("10.0.0.1/24").unwrap();
        let test_cont_addr1 = IpNet::from_str("10.10.0.1/32").unwrap();
        add_addr(&host_side1, &test_addr1).await.unwrap();
        add_addr("c0", &test_cont_addr1).await.unwrap();
        link_up(&host_side1).await.unwrap();

        let dst = IpNet::from_str("10.10.0.1/32").unwrap();

        add_route_in_table(
            &dst,
            None,
            &host_side1,
            RouteScope::Universe,
            CNI_ROUTE_TABLE_ID,
        )
        .await
        .unwrap();

        let r_l = route_list(IpAddr::from_str("127.0.0.1").unwrap(), CNI_ROUTE_TABLE_ID)
            .await
            .unwrap();

        assert_eq!(r_l.len(), 1);

        let res_route = r_l.get(&host_side1).unwrap();

        assert_eq!(res_route, &test_cont_addr1.addr());
    }
}
