pub(crate) mod bgp;
pub(crate) mod cmd;
pub(crate) mod error;
pub(crate) mod fib;
pub(crate) mod proto;
pub(crate) mod rpc;
pub(crate) mod util;

use std::net::Ipv4Addr;

use bgp::cmd::Scope;
use clap::Parser;
use cmd::{Cmd, SubCmd};
use fib::channel::ChannelCmd;

#[tokio::main]
async fn main() {
    let command = Cmd::parse();

    let format = command.format;
    let endpoint = command.endpoint;

    match command.sub {
        SubCmd::Bgp(sub) => match sub.scope {
            Scope::Global(g) => match g.action {
                bgp::global::Action::Get => bgp::global::get(&endpoint, format).await.unwrap(),
                bgp::global::Action::Set { asn, router_id } => {
                    if let Some(router_id) = router_id.as_ref() {
                        let _: Ipv4Addr = router_id.parse().unwrap();
                    }
                    bgp::global::set(&endpoint, asn, router_id).await.unwrap();
                }
                bgp::global::Action::Rib(r) => match r.action {
                    bgp::rib::Action::Add {
                        prefixes,
                        attributes,
                    } => bgp::rib::add(&endpoint, r.afi, r.safi, prefixes, attributes)
                        .await
                        .unwrap(),
                    bgp::rib::Action::Get => bgp::rib::get(&endpoint, format, r.afi, r.safi)
                        .await
                        .unwrap(),
                    bgp::rib::Action::Del { prefixes } => {
                        bgp::rib::del(&endpoint, r.afi, r.safi, prefixes)
                            .await
                            .unwrap()
                    }
                },
                bgp::global::Action::Health => bgp::global::health(&endpoint).await.unwrap(),
                _ => unimplemented!(),
            },
            Scope::Neighbor(n) => match n.action {
                bgp::neighbor::Action::Add {
                    addr,
                    r#as: asn,
                    passive,
                } => bgp::neighbor::add(&endpoint, &addr, asn, passive)
                    .await
                    .unwrap(),
                bgp::neighbor::Action::Get { addr } => {
                    bgp::neighbor::get(&endpoint, format, &addr).await.unwrap()
                }
                bgp::neighbor::Action::List => {
                    bgp::neighbor::list(&endpoint, format).await.unwrap()
                }
                bgp::neighbor::Action::Del { addr } => {
                    bgp::neighbor::delete(&endpoint, &addr).await.unwrap()
                }
                bgp::neighbor::Action::Rib {
                    addr,
                    kind,
                    afi,
                    safi,
                } => bgp::neighbor::rib(&endpoint, format, addr, kind, afi, safi)
                    .await
                    .unwrap(),
                _ => unimplemented!(),
            },
        },
        SubCmd::Fib(sub) => match sub.scope {
            fib::cmd::Scope::Channel(ch) => {
                match ch.action {
                    fib::channel::Action::Get { name } => {
                        fib::channel::get(&endpoint, format, name).await.unwrap()
                    },
                    fib::channel::Action::List => {
                        fib::channel::list(&endpoint, format).await.unwrap()
                    },
                    fib::channel::Action::Route(r) => {
                        fib::route::get(&endpoint, format, r.name).await.unwrap()
                    }
                }
            }
        },
    }
}
