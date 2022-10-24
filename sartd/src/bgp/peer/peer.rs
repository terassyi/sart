use std::fmt::format;
use std::io;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::FutureExt;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::time::{interval_at, Instant, Interval};

use crate::bgp::config::NeighborConfig;
use crate::bgp::error::Error;
use crate::bgp::event::{AdministrativeEvent, Event, TcpConnectionEvent};
use crate::bgp::server::Bgp;

use super::fsm::State;
use super::neighbor;
use super::{fsm::FiniteStateMachine, neighbor::Neighbor};

#[derive(Debug, Clone)]
pub(crate) struct PeerConfig {
    neighbor: Neighbor,
    hold_time: u64,
    keepalive_time: u64,
    connect_retry_time: u64,
}

impl PeerConfig {
    pub fn new(neighbor: NeighborConfig) -> Self {
        Self {
            neighbor: Neighbor::from(neighbor),
            hold_time: Bgp::DEFAULT_HOLD_TIME,
            keepalive_time: Bgp::DEFAULT_KEEPALIVE_TIME,
            connect_retry_time: Bgp::DEFAULT_CONNECT_RETRY_TIME,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Peer {
    config: Arc<Mutex<PeerConfig>>,
    fsm: Mutex<FiniteStateMachine>,
    stream: Arc<Option<TcpStream>>,
    admin_rx: UnboundedReceiver<Event>,
    keepalive_timer: Interval,
    connect_retry_timer: Interval,
    hold_timer: Interval,
}

impl Peer {
    pub fn new(config: Arc<Mutex<PeerConfig>>, rx: UnboundedReceiver<Event>) -> Self {
        let hold_time = { config.lock().unwrap().hold_time };
        let keepalive_time = { config.lock().unwrap().keepalive_time };
        let connect_retry_time = { config.lock().unwrap().connect_retry_time };
        Self {
            config,
            fsm: Mutex::new(FiniteStateMachine::new()),
            stream: Arc::new(None),
            admin_rx: rx,
            hold_timer: interval_at(
                Instant::now() + Duration::new(u32::MAX.into(), 0),
                Duration::from_secs(hold_time),
            ),
            keepalive_timer: interval_at(
                Instant::now() + Duration::new(u32::MAX.into(), 0),
                Duration::from_secs(keepalive_time),
            ),
            connect_retry_timer: interval_at(
                Instant::now() + Duration::new(u32::MAX.into(), 0),
                Duration::from_secs(connect_retry_time),
            ),
        }
    }

    fn state(&self) -> State {
        self.fsm.lock().unwrap().get_state()
    }

    pub async fn handle(&mut self) {
        // handle event

        loop {
            futures::select_biased! {
                // timer handling
                _ = self.hold_timer.tick().fuse() => {

                }
                _ = self.connect_retry_timer.tick().fuse() => {}
                _ = self.keepalive_timer.tick().fuse() => {}
                // event handling
                event = self.admin_rx.recv().fuse() => {
                    if let Some(event) = event {
                        self.handle_event(event).unwrap();
                    }
                }
                // message handling
            };
        }
    }

    fn handle_event(&mut self, event: Event) -> Result<(), Error> {
        println!("{:?}", event);
        match event {
            Event::Admin(event) => match event {
                AdministrativeEvent::ManualStart => self.admin_manual_start().unwrap(),
                _ => unimplemented!(),
            },
            Event::Connection(event) => match event {
                TcpConnectionEvent::TcpCRAcked(stream) => {
                    println!("  {:?}", stream.peer_addr());
                },
                TcpConnectionEvent::TcpConnectionConfirmed(stream) => {
                    println!("  {:?}", stream.peer_addr());
                },
                TcpConnectionEvent::TcpConnectionFail => {},
                _ => unimplemented!(),
            }
            _ => panic!("unimplemented"),
        }
        Ok(())
    }

    fn admin_manual_start(&mut self) -> Result<(), Error> {
        match self.state() {
            State::Idle => {
                self.connect_retry_timer.reset();
                self.fsm.lock().unwrap().mv(Event::AMDIN_MANUAL_START);
            },
            _ => {},
        }
        Ok(())
    }

    fn tcp_connection_acked(&self, event: TcpConnectionEvent) -> Result<(), Error> {

        Ok(())
    }

}

#[derive(Debug)]
pub(crate) struct PeerManager {
    // config: Arc<Mutex<PeerConfig>>,
    pub event_tx: UnboundedSender<Event>,
}

impl PeerManager {
    pub fn new(event_tx: UnboundedSender<Event>) -> Self {
        Self { event_tx }
    }
}
