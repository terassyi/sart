use std::{net::IpAddr, str::FromStr};

use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use netlink_packet_route::route::RouteScope;
use sartd_proto::sart::{CniResult, Interface, IpConf, RouteConf};

use super::{
    error::Error,
    netlink::{self, ContainerLinkInfo, DEFAULT_ROUTE_IPV4, DEFAULT_ROUTE_IPV6},
    netns::NetNS,
};

// K8S_POD_INFRA_CONTAINER_ID=0a6a4b09df59d64e3be5cf662808076fee664447a1c90dd05a5d5588e2cd6b5a;K8S_POD_UID=b0e1fc4a-f842-4ec2-8e23-8c0c8da7b5e5;IgnoreUnknown=1;K8S_POD_NAMESPACE=kube-system;K8S_POD_NAME=coredns-787d4945fb-7xrrd
const K8S_POD_INFRA_CONTAINER_ID: &str = "K8S_POD_INFRA_CONTAINER_ID";
const K8S_POD_UID: &str = "K8S_POD_UID";
const K8S_POD_NAMESPACE: &str = "K8S_POD_NAMESPACE";
const K8S_POD_NAME: &str = "K8S_POD_NAME";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodInfo {
    pub container_id: String,
    pub uid: String,
    pub namespace: String,
    pub name: String,
}

impl FromStr for PodInfo {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut info = PodInfo {
            container_id: String::new(),
            uid: String::new(),
            namespace: String::new(),
            name: String::new(),
        };
        let split_str = s.split(';');
        for kv in split_str.into_iter() {
            let split_kv: Vec<&str> = kv.split('=').collect();
            if split_kv.len() != 2 {
                continue;
            }
            let key = split_kv[0];
            let value = split_kv[1];
            match key {
                K8S_POD_INFRA_CONTAINER_ID => info.container_id = value.to_string(),
                K8S_POD_UID => info.uid = value.to_string(),
                K8S_POD_NAMESPACE => info.namespace = value.to_string(),
                K8S_POD_NAME => info.name = value.to_string(),
                _ => {}
            }
        }
        if info.container_id.is_empty() {
            return Err(Error::MissingField(K8S_POD_INFRA_CONTAINER_ID.to_string()));
        }
        if info.uid.is_empty() {
            return Err(Error::MissingField(K8S_POD_UID.to_string()));
        }
        if info.namespace.is_empty() {
            return Err(Error::MissingField(K8S_POD_NAMESPACE.to_string()));
        }
        if info.name.is_empty() {
            return Err(Error::MissingField(K8S_POD_NAME.to_string()));
        }
        Ok(info)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodAllocation {
    pub container_id: String,
    pub block: String,
    pub addr: IpNet,
}

#[tracing::instrument()]
pub async fn setup_links(
    link_info: &ContainerLinkInfo,
    host_ns: &NetNS,
    container_ns: &NetNS,
    container_addr: &IpNet,
    host_addr: &IpAddr,
    table: u32,
) -> Result<CniResult, Error> {
    // enter into container netns
    container_ns.enter().map_err(Error::NetNS)?;

    // create veth pair in the container netns
    let host_ifname = netlink::add_veth_pair(&link_info.id, &link_info.ifname)
        .await
        .map_err(Error::Netlink)?;

    // move the host side interface to the host netns
    netlink::move_netns(&host_ifname, host_ns)
        .await
        .map_err(Error::Netlink)?;

    // up a container side interface
    netlink::link_up(&link_info.ifname)
        .await
        .map_err(Error::Netlink)?;

    // get the container interface's mac address
    let container_mac = netlink::get_link_mac(&link_info.ifname)
        .await
        .map_err(Error::Netlink)?;

    // exit from the container netns
    // and enter the host netns
    host_ns.enter().map_err(Error::NetNS)?;

    // set container-id as alias
    let alias = link_info.to_alias();
    netlink::set_alias(&host_ifname, &alias)
        .await
        .map_err(Error::Netlink)?;

    // add address(host node address) to the host side interface
    let host_net = match host_addr {
        IpAddr::V4(addr) => IpNet::V4(
            Ipv4Net::new(*addr, 32).map_err(|_| Error::InvalidAddress(host_addr.to_string()))?,
        ),
        IpAddr::V6(addr) => IpNet::V6(
            Ipv6Net::new(*addr, 128).map_err(|_| Error::InvalidAddress(host_addr.to_string()))?,
        ),
    };
    netlink::add_addr(&host_ifname, &host_net)
        .await
        .map_err(Error::Netlink)?;

    // up the host side interface
    netlink::link_up(&host_ifname)
        .await
        .map_err(Error::Netlink)?;

    // add the route to the container in the specified routing table
    netlink::add_route_in_table(
        container_addr,
        None,
        &host_ifname,
        RouteScope::Universe,
        table,
    )
    .await
    .map_err(Error::Netlink)?;

    // enter the container netns again
    container_ns.enter().map_err(Error::NetNS)?;

    // add an address to the container interface
    netlink::add_addr(&link_info.ifname, container_addr)
        .await
        .map_err(Error::Netlink)?;

    // add a route to the container netns
    netlink::add_route(&host_net, None, &link_info.ifname, RouteScope::Link)
        .await
        .map_err(Error::Netlink)?;

    // add a default route to the container netns
    let default_route = match host_net {
        IpNet::V4(_) => DEFAULT_ROUTE_IPV4
            .parse::<IpNet>()
            .map_err(|_| Error::InvalidAddress(DEFAULT_ROUTE_IPV4.to_string()))?,
        IpNet::V6(_) => DEFAULT_ROUTE_IPV6
            .parse::<IpNet>()
            .map_err(|_| Error::InvalidAddress(DEFAULT_ROUTE_IPV6.to_string()))?,
    };
    netlink::add_route(
        &default_route,
        Some(*host_addr),
        &link_info.ifname,
        RouteScope::Universe,
    )
    .await
    .map_err(Error::Netlink)?;

    tracing::info!("Add the route in the container");

    // return into the host netns
    host_ns.enter().map_err(Error::NetNS)?;

    Ok(CniResult {
        interfaces: vec![Interface {
            name: link_info.ifname.clone(),
            mac: container_mac,
            sandbox: container_ns.path().as_path().display().to_string(),
        }],
        ips: vec![IpConf {
            interface: 0,
            address: container_addr.to_string(),
            gateway: host_addr.to_string(),
        }],
        routes: vec![RouteConf {
            dst: default_route.to_string(),
            gw: host_addr.to_string(),
            mtu: 0,
            advmss: 0,
        }],
        dns: None,
    })
}

pub async fn cleanup_links(
    container_id: &str,
    host_ns: &NetNS,
    container_ns: Option<NetNS>,
    container_addr: &IpNet,
    host_addr: &IpAddr,
    container_ifname: &str,
    table: u32,
) -> Result<CniResult, Error> {
    // enter the container netns
    let container_mac = if let Some(ref container_ns) = container_ns {
        container_ns.enter().map_err(Error::NetNS)?;
        // get the container interface's mac address
        netlink::get_link_mac(container_ifname)
            .await
            .map_err(Error::Netlink)?
    } else {
        String::new()
    };

    // enter the host netns
    host_ns.enter().map_err(Error::NetNS)?;

    // delete routes in host netns
    netlink::del_route_in_table(container_addr, table)
        .await
        .map_err(Error::Netlink)?;

    // delete link
    if container_id.len() < 8 {
        return Err(Error::Netlink(netlink::Error::InvalidContainerId));
    }

    let short_id = &container_id[..8];
    let host_ifname = format!("veth{short_id}");
    netlink::del_link(&host_ifname)
        .await
        .map_err(Error::Netlink)?;

    let default_route = match container_addr {
        IpNet::V4(_) => DEFAULT_ROUTE_IPV4
            .parse::<IpNet>()
            .map_err(|_| Error::InvalidAddress(DEFAULT_ROUTE_IPV4.to_string()))?,
        IpNet::V6(_) => DEFAULT_ROUTE_IPV6
            .parse::<IpNet>()
            .map_err(|_| Error::InvalidAddress(DEFAULT_ROUTE_IPV6.to_string()))?,
    };

    Ok(CniResult {
        interfaces: vec![Interface {
            name: container_ifname.to_string(),
            mac: container_mac,
            sandbox: match container_ns {
                Some(container_ns) => container_ns.path().as_path().display().to_string(),
                None => String::new(),
            },
        }],
        ips: vec![IpConf {
            interface: 0,
            address: container_addr.to_string(),
            gateway: host_addr.to_string(),
        }],
        routes: vec![RouteConf {
            dst: default_route.to_string(),
            gw: host_addr.to_string(),
            mtu: 0,
            advmss: 0,
        }],
        dns: None,
    })
}

#[cfg(test)]
mod tests {

    use tests::netlink::{
        add_rule, del_rule, get_link_by_container_id, get_link_index_by_name, route_list,
    };

    use crate::agent::cni::{netns::get_current_netns, server::CNI_ROUTE_TABLE_ID};

    use super::*;

    struct TestContainer {
        id: String,
        netns: NetNS,
        ns_name: String,
        container_addr: IpNet,
        container_ifname: String,
        host_addr: IpAddr,
        host_route_table_id: u32,
    }

    impl TestContainer {
        fn new(container_addr: IpNet, host_addr: IpAddr) -> Self {
            use rand::Rng;
            const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
            let mut rng = rand::thread_rng();

            let container_id: String = (0..32)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect();

            let ns_id: String = (0..8)
                .map(|_| {
                    let idx = rng.gen_range(0..CHARSET.len());
                    CHARSET[idx] as char
                })
                .collect();

            let ns_name = format!("test-{ns_id}");
            let mut cmd = std::process::Command::new("ip");

            cmd.args(["netns", "add", &ns_name]);
            cmd.output().unwrap();
            let ns = NetNS::try_from(format!("/var/run/netns/{}", ns_name).as_str()).unwrap();
            TestContainer {
                id: container_id,
                netns: ns,
                ns_name,
                container_addr,
                container_ifname: "eth0".to_string(),
                host_addr,
                host_route_table_id: 160,
            }
        }
    }

    impl Drop for TestContainer {
        fn drop(&mut self) {
            let mut cmd = std::process::Command::new("ip");

            cmd.args(["netns", "del", &self.ns_name]);
            cmd.output().unwrap();
        }
    }

    #[test]
    fn test_pod_info_from_str() {
        let s = "K8S_POD_INFRA_CONTAINER_ID=0a6a4b09df59d64e3be5cf662808076fee664447a1c90dd05a5d5588e2cd6b5a;K8S_POD_UID=b0e1fc4a-f842-4ec2-8e23-8c0c8da7b5e5;IgnoreUnknown=1;K8S_POD_NAMESPACE=kube-system;K8S_POD_NAME=coredns-787d4945fb-7xrrd";
        let expected = PodInfo {
            container_id: "0a6a4b09df59d64e3be5cf662808076fee664447a1c90dd05a5d5588e2cd6b5a"
                .to_string(),
            uid: "b0e1fc4a-f842-4ec2-8e23-8c0c8da7b5e5".to_string(),
            namespace: "kube-system".to_string(),
            name: "coredns-787d4945fb-7xrrd".to_string(),
        };
        let info = PodInfo::from_str(s).unwrap();
        assert_eq!(expected, info);
    }

    #[tokio::test]
    async fn test_setup_and_cleanup_links() {
        let host_ns = get_current_netns().unwrap();
        let host_addr = IpAddr::from_str("10.10.0.2").unwrap();
        let container_addr = IpNet::from_str("10.0.0.1/32").unwrap();

        add_rule(160, host_addr).await.unwrap();

        let container = TestContainer::new(container_addr, host_addr);
        let link_info = ContainerLinkInfo {
            id: container.id.clone(),
            ifname: container.container_ifname.clone(),
            pool: String::new(),
        };

        let cni_result = setup_links(
            &link_info,
            &host_ns,
            &container.netns,
            &container_addr,
            &host_addr,
            container.host_route_table_id,
        )
        .await
        .unwrap();

        assert_eq!(cni_result.interfaces.len(), 1);
        assert_eq!(cni_result.ips.len(), 1);
        assert_eq!(cni_result.routes.len(), 1);

        let (conn, handle, _) = rtnetlink::new_connection().unwrap();
        tokio::spawn(conn);

        let _exist = get_link_by_container_id(&handle, &container.id)
            .await
            .unwrap();

        let mut cmd = std::process::Command::new("ping");
        cmd.args(["-c", "1", container_addr.addr().to_string().as_str()]);
        let out = cmd.output().unwrap();
        assert!(out.status.success());

        let r_l = route_list(IpAddr::from_str("127.0.0.1").unwrap(), 160)
            .await
            .unwrap();

        cleanup_links(
            &container.id,
            &host_ns,
            None,
            &container_addr,
            &host_addr,
            &container.container_ifname,
            container.host_route_table_id,
        )
        .await
        .unwrap();

        let should_err = get_link_by_container_id(&handle, &container.id).await;
        assert!(should_err.is_err());

        del_rule(160, &host_addr).await.unwrap();
    }
}
