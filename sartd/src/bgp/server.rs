use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use socket2::{Domain, Socket, Type};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{channel, Receiver};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio::sync::oneshot;
use tokio::sync::Notify;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic_reflection::server::Builder;

use crate::bgp::api_server::api;
use crate::bgp::api_server::ApiServer;
use crate::bgp::config::Config;
use crate::bgp::error::Error;
use crate::bgp::family::Afi;
use crate::proto::sart::bgp_api_server::BgpApiServer;

use super::config::NeighborConfig;
use super::error::ControlError;
use super::event::{AdministrativeEvent, ControlEvent, Event};
use super::peer::peer::{Peer, PeerKey, PeerManager};

#[derive(Debug)]
pub(crate) struct Bgp {
    pub config: Arc<Mutex<Config>>,
    pub peer_managers: HashMap<PeerKey, PeerManager>,
    api_server_port: u16,
    event_signal: Arc<Notify>,
}

impl Bgp {
    pub const BGP_PORT: u16 = 179;
    const API_SERVER_PORT: u16 = 5000;

    pub fn new(conf: Config) -> Self {
        Self {
            config: Arc::new(Mutex::new(conf)),
            api_server_port: Self::API_SERVER_PORT,
            peer_managers: HashMap::new(),
            event_signal: Arc::new(Notify::new()),
        }
    }

    pub async fn serve(conf: Config) {
        let mut server = Self::new(conf);

        // let (ctrl_tx, ctrl_rx) = tokio::sync::mpsc::unbounded_channel::<ControlEvent>();
        let (ctrl_tx, mut ctrl_rx) = tokio::sync::mpsc::channel::<ControlEvent>(128);

        let init_tx = ctrl_tx.clone();

        let signal_api = server.event_signal.clone();
        let api_server_port = server.api_server_port;
        println!("--");
        tokio::spawn(async move {
            println!("hogehoge");
            let sock_addr = format!("0.0.0.0:{}", api_server_port).parse().unwrap();

            let reflection_server = Builder::configure()
                .register_encoded_file_descriptor_set(api::FILE_DESCRIPTOR_SET)
                .build()
                .unwrap();

            Server::builder()
                .add_service(BgpApiServer::new(ApiServer::new(ctrl_tx, signal_api)))
                .add_service(reflection_server)
                .serve(sock_addr)
                .await
                .unwrap();
        });
        println!("API server is running at 0.0.0.0:{}", api_server_port);

        let ipv4_listener = {
            create_tcp_listener(
                "0.0.0.0".to_string(),
                server.config.lock().unwrap().port,
                Afi::IPv4,
            )
            .expect("cannot bind Ipv4 tcp listener")
        };
        let ipv6_listener = {
            create_tcp_listener(
                "[::]".to_string(),
                server.config.lock().unwrap().port,
                Afi::IPv6,
            )
            .expect("cannot bind ipv6 listener")
        };
        let listeners = vec![ipv4_listener, ipv6_listener];
        let mut listener_streams = listeners
            .into_iter()
            .map(|l| TcpListenerStream::new(TcpListener::from_std(l).unwrap()))
            .collect::<Vec<TcpListenerStream>>();
        let port = { server.config.lock().unwrap().port };

        println!(
            "Start listening connections at 0.0.0.0:{} and [::]:{}",
            port, port
        );
        let init_event: Vec<ControlEvent> = { server.config.lock().unwrap().get_control_event() };

        tokio::spawn(async move {
            while let Some(event) = ctrl_rx.recv().await {
                server.handle_event(event);
            }
        });

        for e in init_event.into_iter() {
            init_tx.send(e).await.unwrap();
        }

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

    fn handle_event(&mut self, event: ControlEvent) {
        match event {
            ControlEvent::Health => println!("health check"),
            ControlEvent::AddPeer(neighbor) => {
                println!("{:?}", neighbor);
                self.add_peer(neighbor).unwrap();
            }
            _ => println!("Undefined control event."),
        }
        self.event_signal.notify_one();
    }

    fn add_peer(&mut self, neighbor: NeighborConfig) -> Result<(), Error> {
        let key = PeerKey::new(neighbor.address, neighbor.as_number);
        if let Some(_) = self.peer_managers.get(&key) {
            return Err(Error::Control(ControlError::PeerAlreadyExists));
        }
        let (tx, rx) = unbounded_channel::<Event>();
        let mut peer = Peer::new(neighbor, rx);
        let manager = PeerManager::new(tx);
        self.peer_managers
            .insert(PeerKey::new(neighbor.address, neighbor.as_number), manager);
        tokio::spawn(async move {
            peer.handle().await;
        });

        if let Some(manager) = self.peer_managers.get(&key) {
            manager
                .event_queue
                .send(Event::Admin(AdministrativeEvent::ManualStart))
                .unwrap();
        }
        // .map_err(|_| Err(Error::InvalidEvent { val: 0 }));
        Ok(())
    }
}

impl TryFrom<Config> for Bgp {
    type Error = Error;
    fn try_from(conf: Config) -> Result<Self, Self::Error> {
        Ok(Self {
            config: Arc::new(Mutex::new(conf)),
            peer_managers: HashMap::new(),
            api_server_port: Self::API_SERVER_PORT,
            event_signal: Arc::new(Notify::new()),
        })
    }
}

fn create_tcp_listener(addr: String, port: u16, proto: Afi) -> io::Result<std::net::TcpListener> {
    println!("{}", addr);
    // let sock_addr = SocketAddr::new(addr.parse().unwrap(), port);
    let sock_addr: SocketAddr = format!("{}:{}", addr, port).parse().unwrap();
    let sock = Socket::new(
        match proto {
            Afi::IPv4 => Domain::IPV4,
            Afi::IPv6 => Domain::IPV6,
        },
        Type::STREAM,
        None,
    )?;

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
}
