use std::collections::HashMap;
use std::fmt::format;
use std::hash::Hash;
use std::io;
use std::sync::{Arc, Mutex};
use std::net::{SocketAddr, Ipv4Addr};
use futures::StreamExt;
use socket2::{Socket, Domain, Type};
use tokio::net::{TcpListener, TcpStream};
use tokio_stream::{wrappers::TcpListenerStream};
use futures::stream::{Stream, FuturesUnordered};

use crate::bgp::config::Config;
use crate::bgp::family::Afi;
use crate::bgp::error::Error;

use super::peer::peer::PeerManager;

#[derive(Debug)]
pub(crate) struct Bgp {
    config: Arc<Mutex<Config>>,
    peer_managers: HashMap<Ipv4Addr, PeerManager>,
}

impl Bgp {
    pub const BGP_PORT: u16 = 179;

    pub fn new(conf: Config) -> Self {
        Self {
            config: Arc::new(Mutex::new(conf)),
            peer_managers: HashMap::new(),
        }
    }

    pub async fn serve(conf: Config) {
        let server = Self::new(conf);

        let ipv4_listener = {
            create_tcp_listener("0.0.0.0".to_string(), server.config.lock().unwrap().port, Afi::IPv4)
                .expect("cannot bind Ipv4 tcp listener")
        };
        let ipv6_listener = {
            create_tcp_listener("[::]".to_string(), server.config.lock().unwrap().port, Afi::IPv6)
                .expect("cannot bind ipv6 listener")
        };
        let listeners = vec![ipv4_listener, ipv6_listener];
        let mut listener_streams = listeners.into_iter().map(|l| TcpListenerStream::new(TcpListener::from_std(l).unwrap()))
                .collect::<Vec<TcpListenerStream>>();
        loop {
            let mut future_streams = FuturesUnordered::new();
            for stream in &mut listener_streams {
                future_streams.push(stream.next());
            }
            futures::select_biased! {
                stream = future_streams.next() => {
                    if let Some(Some(Ok(stream))) = stream {
                        let sock = stream.peer_addr().unwrap();
                        println!("{:?}", sock);
                    }
                }
            }
        }
    }

    fn prepare_peer(&self) -> Result<(), Error> {

        Ok(())
    }
}

impl TryFrom<Config> for Bgp {
    type Error = Error;
    fn try_from(conf: Config) -> Result<Self, Self::Error> {
        Ok(Self {
            config: Arc::new(Mutex::new(conf)),
            peer_managers: HashMap::new(),
        })
    }
}

fn create_tcp_listener(addr: String, port: u16, proto: Afi) -> io::Result<std::net::TcpListener> {
    println!("{}", addr);
    // let sock_addr = SocketAddr::new(addr.parse().unwrap(), port);
    let sock_addr: SocketAddr = format!("{}:{}", addr, port).parse().unwrap();
    let sock = Socket::new(match proto {
        Afi::IPv4 => Domain::IPV4,
        Afi::IPv6 => Domain::IPV6,
    }, Type::STREAM, None)?;

    if sock_addr.is_ipv6() {
        sock.set_only_v6(true)?;
    }

    sock.set_reuse_address(true)?;
    sock.set_reuse_port(true)?;
    sock.set_nonblocking(true)?;

    sock.bind(&sock_addr.into())?;
    sock.listen(4096)?;
    Ok(sock.into())
}

pub(crate) fn start(conf: Config) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(Bgp::serve(conf));

    println!("sartd bgp server is now starting!");

    loop {}
}
