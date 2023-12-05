use std::net::IpAddr;

use clap::{Parser, Subcommand};
use tabled::Table;

use crate::{
    bgp::rib::DisplayedPath,
    cmd::Output,
    data::bgp::{Path, Paths, Peers},
    error::Error,
    proto::sart::{
        AddPeerRequest, AddressFamily, DeletePeerRequest, GetNeighborPathRequest,
        GetNeighborRequest, ListNeighborRequest, Peer,
    },
    rpc::connect_bgp,
};

use super::rib::{Afi, Kind, Safi};

#[derive(Debug, Clone, Parser)]
pub(crate) struct NeighborCmd {
    #[structopt(subcommand)]
    pub action: Action,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Action {
    Get {
        addr: String,
    },
    List,
    Add {
        addr: String,
        #[arg()]
        r#as: u32,
        #[arg(long, help = "Set passive open flag")]
        passive: bool,
        #[arg(long, help = "Name of BGP peer")]
        name: Option<String>,
    },
    Del {
        addr: String,
    },
    Rib {
        addr: String,
        #[arg(
            value_enum,
            short,
            long,
            default_value = "in",
            help = "in = Input table, out = Output table"
        )]
        kind: Kind,

        #[arg(
            value_enum,
            short,
            long,
            global = true,
            default_value = "ipv4",
            help = "Address Family Information"
        )]
        afi: Afi,
        #[arg(
            value_enum,
            short,
            long,
            global = true,
            default_value = "unicast",
            help = "Sub Address Family Information"
        )]
        safi: Safi,
    },
    Policy,
}

pub(crate) async fn get(endpoint: &str, format: Output, addr: &str) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;

    let _: IpAddr = addr.parse().unwrap();
    let res = client
        .get_neighbor(GetNeighborRequest {
            addr: addr.to_string(),
        })
        .await
        .map_err(Error::FailedToGetResponse)?;

    match format {
        Output::Json => {
            let peer = match &res.get_ref().peer {
                Some(peer) => peer,
                None => return Err(Error::InvalidRPCResponse),
            };
            let data = crate::data::bgp::Peer::try_from(peer)?;
            let json_data = serde_json::to_string_pretty(&data).map_err(Error::Serialize)?;
            println!("{json_data}");
        }
        Output::Plain => {
            let peer = match &res.get_ref().peer {
                Some(peer) => peer,
                None => return Err(Error::InvalidRPCResponse),
            };
            let uptime = match &peer.uptime {
                Some(uptime) => uptime,
                None => return Err(Error::InvalidRPCResponse),
            };
            println!("BGP Neighbor {}, ", peer.address);
            println!("  Name {}", peer.name);
            println!("  Remote AS {}", peer.asn);
            println!("  Version: 4");
            println!("  Router Id: {}", peer.router_id);
            println!("  State: {}", state(peer.state as u8));
            println!("  Uptime: {}", uptime);
            println!("  Hold Time: {} seconds", peer.hold_time);
            println!("  Keep Alive Time: {} seconds", peer.keepalive_time);
            println!();

            println!("  Capabilities");
            println!("    Acceptable Address Family:");
            for family in peer.families.iter() {
                println!("      {:?}", family);
            }

            match &peer.send_counter {
                Some(send_counter) => {
                    let recv_counter = match &peer.recv_counter {
                        Some(recv_counter) => recv_counter,
                        None => {
                            return Err(Error::MissingArgument {
                                msg: "receive counter".to_string(),
                            })
                        }
                    };
                    println!("  Message Statistics:");
                    println!("                 {0: >8} {1: >8}", "Send", "Recv");
                    println!(
                        "    {0: <12} {1: >8} {2: >8}",
                        "Open", send_counter.open, recv_counter.open
                    );
                    println!(
                        "    {0: <12} {1: >8} {2: >8}",
                        "Update", send_counter.update, recv_counter.update
                    );
                    println!(
                        "    {0: <12} {1: >8} {2: >8}",
                        "Keepalive", send_counter.keepalive, recv_counter.keepalive
                    );
                    println!(
                        "    {0: <12} {1: >8} {2: >8}",
                        "Notification", send_counter.notification, recv_counter.notification
                    );
                    println!(
                        "    {0: <12} {1: >8} {2: >8}",
                        "RouteRefresh", send_counter.route_refresh, recv_counter.route_refresh
                    );
                }
                None => {}
            }
        }
    }
    Ok(())
}

pub(crate) async fn list(endpoint: &str, format: Output) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;
    let res = client
        .list_neighbor(ListNeighborRequest {})
        .await
        .map_err(Error::FailedToGetResponse)?;

    match format {
        Output::Json => {
            let mut peers = Vec::new();
            for p in res
                .get_ref()
                .peers
                .iter()
                .map(crate::data::bgp::Peer::try_from)
            {
                match p {
                    Ok(p) => peers.push(p),
                    Err(e) => return Err(e),
                }
            }
            let data = Peers { peers };
            let json_data = serde_json::to_string_pretty(&data).map_err(Error::Serialize)?;
            println!("{json_data}");
        }
        Output::Plain => {
            println!("BGP Neighbor List\n");
            for peer in res.get_ref().peers.iter() {
                println!("  BGP Neighbor {}, ", peer.address);
                println!("    Name {}", peer.name);
                println!("    Remote AS {}", peer.asn);
                println!("    Version: 4");
                println!("    Router Id: {}", peer.router_id);
                println!("    State: {}", state(peer.state as u8));
                println!("    Hold Time: {} seconds", peer.hold_time);
                println!("    Keep Alive Time: {} seconds", peer.keepalive_time);
                println!();
            }
        }
    }
    Ok(())
}

pub(crate) async fn add(
    endpoint: &str,
    name: Option<String>,
    addr: &str,
    asn: u32,
    passive: bool,
) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;

    let address: IpAddr = addr.parse().unwrap();

    let name = match name {
        Some(n) => n,
        None => format!("{}/{}", asn, addr),
    };

    let _res = client
        .add_peer(AddPeerRequest {
            peer: Some(Peer {
                name,
                asn,
                address: address.to_string(),
                router_id: "0.0.0.0".to_string(),
                families: Vec::new(),
                hold_time: 0,
                keepalive_time: 0,
                uptime: None,
                send_counter: None,
                recv_counter: None,
                state: 1,
                passive_open: passive,
            }),
        })
        .await
        .map_err(Error::FailedToGetResponse)?;
    Ok(())
}

pub(crate) async fn delete(endpoint: &str, addr: &str) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;

    let _: IpAddr = addr.parse().unwrap();

    let _res = client
        .delete_peer(DeletePeerRequest {
            addr: addr.to_string(),
        })
        .await
        .map_err(Error::FailedToGetResponse)?;
    Ok(())
}

pub(crate) async fn rib(
    endpoint: &str,
    format: Output,
    addr: String,
    kind: Kind,
    afi: Afi,
    safi: Safi,
) -> Result<(), Error> {
    let mut client = connect_bgp(endpoint).await;
    let _address: IpAddr = addr.parse().unwrap();
    let res = client
        .get_neighbor_path(GetNeighborPathRequest {
            kind: kind as i32,
            addr,
            family: Some(AddressFamily {
                afi: afi as i32,
                safi: safi as i32,
            }),
        })
        .await
        .map_err(Error::FailedToGetResponse)?;

    match format {
        Output::Json => {
            let data = Paths {
                paths: res.get_ref().paths.iter().map(Path::from).collect(),
            };
            let json_data = serde_json::to_string_pretty(&data).map_err(Error::Serialize)?;
            println!("{json_data}");
        }
        Output::Plain => {
            let display: Vec<DisplayedPath> = res
                .get_ref()
                .paths
                .iter()
                .map(DisplayedPath::from)
                .collect();
            let data = Table::new(display).to_string();
            println!("{}", data);
        }
    }
    Ok(())
}

fn state(n: u8) -> &'static str {
    match n {
        1 => "IDLE",
        2 => "CONNECT",
        3 => "ACTIVE",
        4 => "OPEN_SENT",
        5 => "OPEN_CONFIRM",
        6 => "ESTABLISHED",
        _ => "UNKNOWN",
    }
}
