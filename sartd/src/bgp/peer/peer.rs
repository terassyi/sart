use std::fmt::format;
use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use futures::stream::{FuturesUnordered, Next};
use futures::{FutureExt, SinkExt, Stream, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, Notify};
use tokio::time::{interval_at, sleep, Instant, Interval, Sleep};
use tokio_util::codec::{Framed, FramedRead};

use crate::bgp::capability::{Capability, CapabilitySet};
use crate::bgp::config::NeighborConfig;
use crate::bgp::error::{Error, PeerError};
use crate::bgp::event::{
    AdministrativeEvent, BgpMessageEvent, Event, TcpConnectionEvent, TimerEvent,
};
use crate::bgp::packet::codec::Codec;
use crate::bgp::packet::message::{Message, MessageBuilder, MessageType};
use crate::bgp::server::Bgp;

use super::fsm::State;
use super::neighbor;
use super::{fsm::FiniteStateMachine, neighbor::Neighbor};

#[derive(Debug)]
pub(crate) struct PeerManager {
    config: Arc<Mutex<PeerConfig>>,
    pub event_tx: UnboundedSender<Event>,
}

impl PeerManager {
    pub fn new(config: Arc<Mutex<PeerConfig>>, event_tx: UnboundedSender<Event>) -> Self {
        Self { config, event_tx }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PeerConfig {
    neighbor: Neighbor,
    asn: u32,
    router_id: Ipv4Addr,
    hold_time: u64,
    keepalive_time: u64,
    connect_retry_time: u64,
    local_capabilities: CapabilitySet,
}

impl PeerConfig {
    pub fn new(
        local_as: u32,
        local_router_id: Ipv4Addr,
        neighbor: NeighborConfig,
        local_capabilities: CapabilitySet,
    ) -> Self {
        Self {
            neighbor: Neighbor::from(neighbor),
            asn: local_as,
            router_id: local_router_id,
            hold_time: Bgp::DEFAULT_HOLD_TIME,
            keepalive_time: Bgp::DEFAULT_KEEPALIVE_TIME,
            connect_retry_time: Bgp::DEFAULT_CONNECT_RETRY_TIME,
            local_capabilities,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Peer {
    config: Arc<Mutex<PeerConfig>>,
    fsm: Mutex<FiniteStateMachine>,
    admin_rx: UnboundedReceiver<Event>,
    msg_tx: Option<UnboundedSender<Message>>,
    msg_event_rx: Option<UnboundedReceiver<BgpMessageEvent>>,
    keepalive_timer: Interval,
    hold_timer: Timer,
    uptime: Instant,
    negotiated_hold_time: u64,
}

impl Peer {
    pub fn new(config: Arc<Mutex<PeerConfig>>, rx: UnboundedReceiver<Event>) -> Self {
        let hold_time = { config.lock().unwrap().hold_time };
        let keepalive_time = { config.lock().unwrap().keepalive_time };
        let connect_retry_time = { config.lock().unwrap().connect_retry_time };
        Self {
            config,
            fsm: Mutex::new(FiniteStateMachine::new()),
            admin_rx: rx,
            msg_tx: None,
            msg_event_rx: None,
            hold_timer: Timer::new(),
            keepalive_timer: interval_at(
                Instant::now() + Duration::new(u32::MAX.into(), 0),
                Duration::from_secs(keepalive_time),
            ),
            uptime: Instant::now(),
            negotiated_hold_time: Bgp::DEFAULT_HOLD_TIME,
        }
    }

    fn state(&self) -> State {
        self.fsm.lock().unwrap().get_state()
    }

    pub async fn handle(&mut self) {
        // handle event
        let (msg_event_tx, mut msg_event_rx) = unbounded_channel::<BgpMessageEvent>();

        loop {
            futures::select_biased! {
                // timer handling
                _ = self.hold_timer.ticker.next() => {
                    println!("hold timer expire");
                    let elapsed = self.hold_timer.last.elapsed().as_secs();
                    if elapsed >= self.hold_timer.interval {
                        println!("correct {}", elapsed);
                    }
                }
                _ = self.keepalive_timer.tick().fuse() => {}
                // event handling
                event = self.admin_rx.recv().fuse() => {
                    if let Some(event) = event {
                        match event {
                            Event::Admin(event) => match event {
                                AdministrativeEvent::ManualStart => self.admin_manual_start().unwrap(),
                                _ => unimplemented!(),
                            },
                            Event::Connection(event) => match event {
                                TcpConnectionEvent::TcpCRAcked(stream) => {
                                    println!("  {:?}", stream.peer_addr());
                                    self.handle_tcp_connect(stream, msg_event_tx.clone(), Event::CONNECTION_TCP_CR_ACKED);
                                },
                                TcpConnectionEvent::TcpConnectionConfirmed(stream) => {
                                    println!("  {:?}", stream.peer_addr());
                                    self.handle_tcp_connect(stream, msg_event_tx.clone(), Event::CONNECTION_TCP_CONNECTION_CONFIRMED);
                                },
                                TcpConnectionEvent::TcpConnectionFail => {},
                                _ => unimplemented!(),
                            },
                            Event::Timer(event) => match event {
                                TimerEvent::ConnectRetryTimerExpire => {},
                                _ => unimplemented!(),
                            },
                            _ => panic!("unhandlable event"),
                        }
                    }
                }
                event = msg_event_rx.recv().fuse() => {

                }
                // message handling
            };
        }
    }

    fn send(&self, msg: Message) -> Result<(), Error> {
        match &self.msg_tx {
            Some(msg_tx) => msg_tx
                .send(msg)
                .map_err(|_| Error::Peer(PeerError::FailedToSendMessage))?,
            None => return Err(Error::Peer(PeerError::ConnectionNotEstablished)),
        }
        Ok(())
    }

    fn initialize(
        &mut self,
        stream: TcpStream,
        msg_event_tx: UnboundedSender<BgpMessageEvent>,
    ) -> Result<(), Error> {
        let mut codec = Framed::new(stream, Codec::new(true, false));
        let (msg_tx, mut msg_rx) = unbounded_channel::<Message>();
        self.msg_tx = Some(msg_tx);
        tokio::spawn(async move {
            println!("message handler process is running");
            let (mut sender, mut receiver) = codec.split();
            loop {
                futures::select_biased! {
                    msg = receiver.next().fuse() => {
                        if let Some(Ok(msg)) = msg {
                            println!("{:?}", msg);
                            msg_event_tx.send(BgpMessageEvent::BgpOpen); // TODO

                        }
                    }
                    msg = msg_rx.recv().fuse() => {
                        if let Some(msg) = msg {
                            sender.send(msg).await;
                        }
                    }
                }
            }
        });
        Ok(())
    }

    fn admin_manual_start(&mut self) -> Result<(), Error> {
        self.fsm.lock().unwrap().mv(Event::AMDIN_MANUAL_START);
        Ok(())
    }

    fn handle_tcp_connect(
        &mut self,
        stream: TcpStream,
        msg_event_tx: UnboundedSender<BgpMessageEvent>,
        event: u8,
    ) -> Result<(), Error> {
        match self.state() {
            State::Idle => {} // ignore this event
            State::Connect => {
                // If the TCP connection succeeds (Event 16 or Event 17), the local
                // system checks the DelayOpen attribute prior to processing.  If the
                // DelayOpen attribute is set to TRUE, the local system:
                //   - stops the ConnectRetryTimer (if running) and sets the
                //     ConnectRetryTimer to zero,
                //   - sets the DelayOpenTimer to the initial value, and
                //   - stays in the Connect state.
                // If the DelayOpen attribute is set to FALSE, the local system:
                //   - stops the ConnectRetryTimer (if running) and sets the
                //     ConnectRetryTimer to zero,
                //   - completes BGP initialization
                //   - sends an OPEN message to its peer,
                //   - sets the HoldTimer to a large value, and
                //   - changes its state to OpenSent.
                self.initialize(stream, msg_event_tx)?;
                let msg = self.build_open_msg()?;
                self.send(msg)?;
                let hold_time = { self.config.lock().unwrap().hold_time };
                self.hold_timer.push(hold_time);
            }
            State::Active => {
                // In response to the success of a TCP connection (Event 16 or Event 17), the local system checks the DelayOpen optional attribute prior to processing.
                // If the DelayOpen attribute is set to TRUE, the local system:
                //   - stops the ConnectRetryTimer and sets the ConnectRetryTimer
                //     to zero,
                //   - sets the DelayOpenTimer to the initial value
                //     (DelayOpenTime), and
                //   - stays in the Active state.
                // If the DelayOpen attribute is set to FALSE, the local system:
                //   - sets the ConnectRetryTimer to zero,
                //   - completes the BGP initialization,
                //   - sends the OPEN message to its peer,
                //   - sets its HoldTimer to a large value, and
                //   - changes its state to OpenSent.
                // A HoldTimer value of 4 minutes is suggested as a "large value" for the HoldTimer.
                self.initialize(stream, msg_event_tx)?;
                let msg = self.build_open_msg()?;
                self.send(msg)?;
                let hold_time = { self.config.lock().unwrap().hold_time };
                self.hold_timer.push(hold_time);
            }
            State::OpenSent => {
                // If a TcpConnection_Valid (Event 14), Tcp_CR_Acked (Event 16), or a
                // TcpConnectionConfirmed event (Event 17) is received, a second TCP
                // connection may be in progress.  This second TCP connection is
                // tracked per Connection Collision processing (Section 6.8) until an
                // OPEN message is received.
            }
            State::OpenConfirm => {
                // In the event of a TcpConnection_Valid event (Event 14), or the
                // success of a TCP connection (Event 16 or Event 17) while in
                // OpenConfirm, the local system needs to track the second
                // connection.
            }
            State::Established => {
                // In response to an indication that the TCP connection is
                // successfully established (Event 16 or Event 17), the second
                // connection SHALL be tracked until it sends an OPEN message.
            }
        }
        self.fsm.lock().unwrap().mv(event);
        Ok(())
    }

    fn hold_timer_expire(&self) -> Result<(), Error> {
        match self.state() {
            State::OpenSent => {}
            State::OpenConfirm => {}
            State::Established => {}
            _ => {
                // release BGP resources
                // drop tcp connection
                // increments ConnectRetryCounter by 1
                // stay
            }
        }
        self.fsm.lock().unwrap().mv(Event::TIMER_HOLD_TIMER_EXPIRE);
        Ok(())
    }

    fn build_open_msg(&self) -> Result<Message, Error> {
        let conf = self.config.lock().unwrap();
        let mut builder = MessageBuilder::builder(MessageType::Open);
        builder
            .asn(conf.asn)?
            .hold_time(conf.hold_time as u16)?
            .identifier(conf.router_id)?;
        let mut caps: Vec<(&u8, &Capability)> = conf.local_capabilities.iter().collect();
        caps.sort_by(|a, b| a.0.cmp(&b.0));
        for cap in caps.iter() {
            builder.capability(&cap.1)?;
        }
        builder.build()
    }
}

#[derive(Debug)]
struct Timer {
    ticker: FuturesUnordered<Sleep>,
    interval: u64,
    last: Instant,
}

impl Timer {
    fn new() -> Self {
        let mut ticker = FuturesUnordered::new();
        ticker.push(sleep(Duration::new(u64::MAX, 0)));
        Self {
            ticker,
            interval: u64::MAX,
            last: Instant::now(),
        }
    }

    fn push(&mut self, t: u64) {
        self.ticker.push(sleep(Duration::new(t, 0)));
        self.interval = t;
        self.last = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::{Arc, Mutex};

    use crate::bgp::server::Bgp;
    use crate::bgp::{
        capability::CapabilitySet,
        config::NeighborConfig,
        packet::{self, message::Message},
    };

    use super::{Peer, PeerConfig};

    #[tokio::test]
    async fn works_peer_build_open_msg() {
        let conf = Arc::new(Mutex::new(PeerConfig::new(
            100,
            Ipv4Addr::new(1, 1, 1, 1),
            NeighborConfig {
                asn: 200,
                address: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
                router_id: Ipv4Addr::new(2, 2, 2, 2),
            },
            CapabilitySet::default(100),
        )));
        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
        let peer = Peer::new(conf, rx);
        let msg = peer.build_open_msg().unwrap();
        assert_eq!(
            Message::Open {
                version: 4,
                as_num: 100,
                hold_time: Bgp::DEFAULT_HOLD_TIME as u16,
                identifier: Ipv4Addr::new(1, 1, 1, 1),
                capabilities: vec![
                    packet::capability::Capability::RouteRefresh,
                    packet::capability::Capability::FourOctetASNumber(100),
                ]
            },
            msg
        );
    }
}
