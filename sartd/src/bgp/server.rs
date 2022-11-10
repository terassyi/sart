use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use socket2::{Domain, Socket, Type};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Add;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{channel, Receiver, UnboundedSender, Sender};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio::sync::Notify;
use tokio::time::{interval_at, sleep, timeout, Instant};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic_reflection::server::Builder;

use crate::bgp::api_server::api;
use crate::bgp::api_server::ApiServer;
use crate::bgp::config::Config;
use crate::bgp::error::Error;
use crate::bgp::event::{TcpConnectionEvent, TimerEvent};
use crate::bgp::family::Afi;
use crate::bgp::peer::fsm::State;
use crate::proto::sart::bgp_api_server::BgpApiServer;

use super::capability::{Capability, CapabilitySet};
use super::config::NeighborConfig;
use super::error::{ControlError, PeerError, RibError};
use super::event::{AdministrativeEvent, ControlEvent, Event, RibEvent};
use super::family::AddressFamily;
use super::peer::neighbor::NeighborPair;
use super::peer::peer::{Peer, PeerInfo, PeerManager};
use super::rib::RibManager;

#[derive(Debug)]
pub(crate) struct Bgp {
    pub config: Arc<Mutex<Config>>,
    pub peer_managers: HashMap<IpAddr, PeerManager>,
    global_capabilities: CapabilitySet,
    families: Vec<AddressFamily>,
    api_server_port: u16,
    connect_retry_time: u64,
    active_conn_rx: UnboundedReceiver<TcpStream>,
    active_conn_tx: UnboundedSender<TcpStream>,
    event_signal: Arc<Notify>,
    rib_event_tx: Sender<RibEvent>,
}

impl Bgp {
    pub const BGP_PORT: u16 = 179;
    pub const DEFAULT_HOLD_TIME: u64 = 180;
    pub const DEFAULT_CONNECT_RETRY_TIME: u64 = 10;
    pub const DEFAULT_KEEPALIVE_TIME: u64 = 60;
    const API_SERVER_PORT: u16 = 5000;

    pub fn new(conf: Config, rib_event_tx: Sender<RibEvent>) -> Self {
        let (active_conn_tx, mut active_conn_rx) =
            tokio::sync::mpsc::unbounded_channel::<TcpStream>();
        let asn = conf.asn;
        Self {
            config: Arc::new(Mutex::new(conf)),
            peer_managers: HashMap::new(),
            global_capabilities: CapabilitySet::default(asn),
            families: vec![AddressFamily::ipv4_unicast(), AddressFamily::ipv6_unicast()],
            connect_retry_time: Self::DEFAULT_CONNECT_RETRY_TIME,
            api_server_port: Self::API_SERVER_PORT,
            event_signal: Arc::new(Notify::new()),
            active_conn_rx,
            active_conn_tx,
            rib_event_tx,
        }
    }

    #[tracing::instrument(skip(conf))]
    pub async fn serve(conf: Config) {
        let rib_endpoint_conf = conf.rib_endpoint.clone();

        let (rib_event_tx, mut rib_event_rx) = channel::<RibEvent>(128);
        let mut rib_manager = RibManager::new(rib_endpoint_conf, vec![Afi::IPv4, Afi::IPv6]); // TODO: enable to disable ipv6 or ipv4 by config or event


        let mut server = Self::new(conf, rib_event_tx);

        let (ctrl_tx, mut ctrl_rx) = channel::<ControlEvent>(128);
        let init_tx = ctrl_tx.clone();

        let signal_api = server.event_signal.clone();
        let api_server_port = server.api_server_port;
        tokio::spawn(async move {
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
        tracing::info!("API server is started at 0.0.0.0:{}", api_server_port);

        let ipv4_listener = {
            create_tcp_listener(
                "0.0.0.0".to_string(),
                Self::BGP_PORT,
                Afi::IPv4,
            )
            .expect("cannot bind Ipv4 tcp listener")
        };
        let ipv6_listener = {
            create_tcp_listener(
                "[::]".to_string(),
                Self::BGP_PORT,
                Afi::IPv6,
            )
            .expect("cannot bind ipv6 listener")
        };
        let listeners = vec![ipv4_listener, ipv6_listener];
        let mut listener_streams = listeners
            .into_iter()
            .map(|l| TcpListenerStream::new(TcpListener::from_std(l).unwrap()))
            .collect::<Vec<TcpListenerStream>>();
        let port = Bgp::BGP_PORT;

        tracing::info!(
            "Sartd BGP server is running at 0.0.0.0:{} and [::]:{}",
            port,
            port
        );
        let init_event: Vec<ControlEvent> = { server.config.lock().unwrap().get_control_event() };

        for e in init_event.into_iter() {
            tracing::info!(level="global",event=?e);
            init_tx.send(e).await.unwrap();
        }

        let mut peer_management_interval = interval_at(Instant::now(), Duration::new(server.connect_retry_time, 0));

        loop {
            let mut future_streams = FuturesUnordered::new();
            for stream in &mut listener_streams {
                future_streams.push(stream.next());
            }
            futures::select_biased! {
                stream = future_streams.next() => {
                    if let Some(Some(stream)) = stream {
                        match stream {
                            Ok(stream) => {
                                let sock = stream.peer_addr().unwrap();
                                if let Some(manager) = server.peer_managers.get(&sock.ip()) {
                                    manager.event_tx.send(Event::Connection(TcpConnectionEvent::TcpConnectionConfirmed(stream))).unwrap();
                                }
                            },
                            Err(e) => {
                                tracing::error!("failed to establish the passive connection: {:?}", e);
                            }
                        };
                    }
                }
                stream = server.active_conn_rx.recv().fuse() => {
                    if let Some(stream) = stream {
                        let sock = stream.peer_addr().unwrap();
                        if let Some(manager) = server.peer_managers.get(&sock.ip()) {
                            match manager.event_tx.send(Event::Connection(TcpConnectionEvent::TcpCRAcked(stream))) {
                                Ok(_) => {},
                                Err(e) => tracing::error!(level="global",error=?e),
                            };
                        }
                    }
                }
                event = ctrl_rx.recv().fuse() => {
                    if let Some(event) = event {
                        match server.handle_event(event).await {
                            Ok(_) => {},
                            Err(e) => tracing::error!(level="global",error=?e),
                        };
                    }
                }
                rib_event = rib_event_rx.recv().fuse() => {
                    if let Some(rib_event) = rib_event {
                        match rib_manager.handle(rib_event) {
                            Ok(_) => {},
                            Err(e) => tracing::error!(level="global",error=?e),
                        }
                    }
                }
                _ = peer_management_interval.tick().fuse() => {
                    for peer_manager in server.peer_managers.values() {
                        let info = peer_manager.info.lock().unwrap();
                        if info.state == State::Idle {
                            tracing::info!(peer.addr=info.neighbor.get_addr().to_string(), peer.asn=info.neighbor.get_asn(), "restart peer");
                            peer_manager.event_tx.send(Event::Admin(AdministrativeEvent::ManualStart)).unwrap();
                        }
                    }
                }
            }
        }
    }

    #[tracing::instrument(skip(self, event))]
    async fn handle_event(&mut self, event: ControlEvent) -> Result<(), Error> {
        tracing::info!(level="global", event=%event);
        match event {
            ControlEvent::Health => {}
            ControlEvent::AddPeer(neighbor) => {
                self.add_peer(neighbor).await?;
            },
            _ => tracing::error!("undefined control event"),
        }
        self.event_signal.notify_one();
        Ok(())
    }

    async fn add_peer(&mut self, neighbor: NeighborConfig) -> Result<(), Error> {
        if let Some(_) = self.peer_managers.get(&neighbor.address) {
            return Err(Error::Control(ControlError::PeerAlreadyExists));
        }
        let (tx, rx) = unbounded_channel::<Event>();
        let (local_as, local_router_id) = {
            let conf = self.config.lock().unwrap();
            (conf.asn, conf.router_id)
        };
        let info = Arc::new(Mutex::new(PeerInfo::new(
            local_as,
            local_router_id,
            neighbor,
            self.global_capabilities.clone(),
            self.families.clone(),
        )));

        let (rib_event_tx, rib_event_rx) = channel(128);

        self.rib_event_tx.send(RibEvent::AddPeer{neighbor: NeighborPair::from(&neighbor), rib_event_tx}).await
            .map_err(|e| {
                tracing::error!(level="peer",error=?e);
                Error::Rib(RibError::ManagerDown)})?;

        let mut peer = Peer::new(info.clone(), rx, self.rib_event_tx.clone(), rib_event_rx);
        let manager = PeerManager::new(info, tx);
        self.peer_managers.insert(neighbor.address, manager);

        // start to handle peer event.
        tokio::spawn(async move {
            peer.handle().await;
        });

        if let Some(manager) = self.peer_managers.get(&neighbor.address) {
            manager
                .event_tx
                .send(Event::Admin(AdministrativeEvent::ManualStart))
                .map_err(|_| Error::Peer(PeerError::Down))?;

            // start to connect to the remote peer
            let active_conn_tx = self.active_conn_tx.clone();
            let event_tx = manager.event_tx.clone();
            let connect_retry_time = self.connect_retry_time;
            tokio::spawn(async move {
                loop {
                    if let Ok(Ok(stream)) = timeout(
                        Duration::from_secs(connect_retry_time / 2),
                        TcpStream::connect(format!("{}:179", neighbor.address)),
                    )
                    .await
                    {
                        match active_conn_tx.send(stream) {
                            Ok(_) => {},
                            Err(e) => tracing::error!(level="global",error=?e),
                        };
                        return;
                    }
                    sleep(Duration::from_secs(connect_retry_time)).await;
                    event_tx
                        .send(Event::Timer(TimerEvent::ConnectRetryTimerExpire))
                        .expect("event channel is not worked.");
                }
            });
        }
        Ok(())
    }
}

fn create_tcp_listener(addr: String, port: u16, proto: Afi) -> io::Result<std::net::TcpListener> {
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
