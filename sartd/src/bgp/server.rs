use futures::stream::FuturesUnordered;
use futures::{FutureExt, StreamExt};
use socket2::{Domain, Socket, TcpKeepalive, Type};
use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::mpsc::unbounded_channel;
use tokio::sync::mpsc::{channel, Sender};
use tokio::sync::Notify;
use tokio::time::{interval_at, Instant};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic_reflection::server::Builder;
use tracing::Level;
use tracing_futures::Instrument;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use crate::bgp::api_server::api;
use crate::bgp::api_server::ApiServer;
use crate::bgp::config::Config;
use crate::bgp::error::Error;
use crate::bgp::event::TcpConnectionEvent;
use crate::bgp::family::Afi;
use crate::bgp::peer::fsm::State;
use crate::proto::sart::bgp_api_server::BgpApiServer;

use super::capability::CapabilitySet;
use super::config::{NeighborConfig, TraceConfig};
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
        let asn = conf.asn;
        Self {
            config: Arc::new(Mutex::new(conf)),
            peer_managers: HashMap::new(),
            global_capabilities: CapabilitySet::default(asn),
            families: vec![AddressFamily::ipv4_unicast(), AddressFamily::ipv6_unicast()],
            connect_retry_time: Self::DEFAULT_CONNECT_RETRY_TIME,
            api_server_port: Self::API_SERVER_PORT,
            event_signal: Arc::new(Notify::new()),
            rib_event_tx,
        }
    }

    pub async fn serve(conf: Config, trace: TraceConfig) {
        prepare_tracing(trace);
        let _enter = tracing::info_span!("bgp").entered();

        let rib_endpoint_conf = conf.rib_endpoint.clone();
        let (rib_event_tx, mut rib_event_rx) = channel::<RibEvent>(128);
        let mut rib_manager = RibManager::new(
            conf.asn,
            conf.router_id,
            rib_endpoint_conf,
            vec![Afi::IPv4, Afi::IPv6],
            false,
        ); // TODO: enable to disable ipv6 or ipv4 by config or event

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
            create_tcp_listener("0.0.0.0".to_string(), Self::BGP_PORT, Afi::IPv4)
                .expect("cannot bind Ipv4 tcp listener")
        };
        let ipv6_listener = {
            create_tcp_listener("[::]".to_string(), Self::BGP_PORT, Afi::IPv6)
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
            init_tx.send(e).await.unwrap();
        }

        let mut peer_management_interval =
            interval_at(Instant::now(), Duration::new(server.connect_retry_time, 0));

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
                                tracing::error!(error=?e,"failed to establish the passive connection");
                            }
                        };
                    }
                }
                event = ctrl_rx.recv().fuse() => {
                    if let Some(event) = event {
                        match server.handle_event(event).await {
                            Ok(_) => {},
                            Err(e) => tracing::error!(error=?e,"failed to handle a control event"),
                        };
                    }
                }
                rib_event = rib_event_rx.recv().fuse() => {
                    if let Some(rib_event) = rib_event {
                        match rib_manager.handle(rib_event)
                            .instrument(tracing::info_span!("rib"))
                            .await {
                            Ok(_) => {},
                            Err(e) => tracing::error!(error=?e,"failed to handle a rib event"),
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
        tracing::info!(event=%event);
        match event {
            ControlEvent::Health => {}
            ControlEvent::AddPeer(neighbor) => {
                self.add_peer(neighbor).in_current_span().await?;
            }
        }
        self.event_signal.notify_one();
        Ok(())
    }

    #[tracing::instrument(skip(self, neighbor), fields(peer.asn = neighbor.asn, peer.addr = neighbor.address.to_string(), peer.id = neighbor.router_id.to_string()))]
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

        let (peer_rib_event_tx, peer_rib_event_rx) = channel(128);

        self.rib_event_tx
            .send(RibEvent::AddPeer {
                neighbor: NeighborPair::from(&neighbor),
                rib_event_tx: peer_rib_event_tx,
            })
            .await
            .map_err(|e| {
                tracing::error!(error=?e);
                Error::Rib(RibError::ManagerDown)
            })?;

        let mut peer = Peer::new(
            info.clone(),
            rx,
            self.rib_event_tx.clone(),
            peer_rib_event_rx,
        );
        let manager = PeerManager::new(info, tx);
        self.peer_managers.insert(neighbor.address, manager);

        // start to handle peer event.
        tokio::spawn(async move {
            peer.handle()
                .instrument(tracing::info_span!(
                    "peer",
                    peer.asn = neighbor.asn,
                    peer.addr = neighbor.address.to_string(),
                    peer.id = neighbor.router_id.to_string(),
                ))
                .await;
        });
        tracing::debug!("start a peer handling loop");

        if let Some(manager) = self.peer_managers.get(&neighbor.address) {
            let start_event = if let Some(passive) = neighbor.passive {
                if passive {
                    AdministrativeEvent::ManualStartWithPassiveTcpEstablishment
                } else {
                    AdministrativeEvent::ManualStart
                }
            } else {
                AdministrativeEvent::ManualStart
            };

            manager
                .event_tx
                .send(Event::Admin(start_event))
                .map_err(|_| Error::Peer(PeerError::Down))?;
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
    sock.set_ttl(1)?;

    let tcp_keepalive = TcpKeepalive::new().with_interval(Duration::new(30, 0));
    sock.set_tcp_keepalive(&tcp_keepalive)?;

    sock.bind(&sock_addr.into())?;
    sock.listen(4096)?;
    Ok(sock.into())
}

pub(crate) fn start(conf: Config, trace: TraceConfig) {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(Bgp::serve(conf, trace));
}

fn prepare_tracing(conf: TraceConfig) {
    // Configure otel exporter.
    if conf.format == "json" {
        tracing_subscriber::Registry::default()
            .with(tracing_subscriber::fmt::Layer::new().with_ansi(true).json())
            .with(tracing_subscriber::filter::LevelFilter::from_str(&conf.level).unwrap())
            .init();
    } else {
        tracing_subscriber::Registry::default()
            .with(tracing_subscriber::fmt::Layer::new().with_ansi(true))
            .with(tracing_subscriber::filter::LevelFilter::from_str(&conf.level).unwrap())
            .init();
    }
}
