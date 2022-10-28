use std::fmt::format;
use std::io;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{FutureExt, SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::{oneshot, Notify};
use tokio::time::{interval_at, Instant, Interval};
use tokio_util::codec::{Framed, FramedRead};

use crate::bgp::config::NeighborConfig;
use crate::bgp::error::Error;
use crate::bgp::event::{
    AdministrativeEvent, BgpMessageEvent, Event, TcpConnectionEvent, TimerEvent,
};
use crate::bgp::packet::codec::Codec;
use crate::bgp::packet::message::Message;
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
    admin_rx: UnboundedReceiver<Event>,
    msg_tx: Option<UnboundedSender<Message>>,
    msg_event_rx: Option<UnboundedReceiver<BgpMessageEvent>>,
    keepalive_timer: Interval,
    hold_timer: Interval,
    uptime: Instant,
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
            hold_timer: interval_at(
                Instant::now() + Duration::new(u32::MAX.into(), 0),
                Duration::from_secs(hold_time),
            ),
            keepalive_timer: interval_at(
                Instant::now() + Duration::new(u32::MAX.into(), 0),
                Duration::from_secs(keepalive_time),
            ),
            uptime: Instant::now(),
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
                _ = self.hold_timer.tick().fuse() => {

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
                                TcpConnectionEvent::TcpCRAcked(stream) | TcpConnectionEvent::TcpConnectionConfirmed(stream) => {
                                    println!("  {:?}", stream.peer_addr());
                                    self.handle_tcp_connect(stream, msg_event_tx.clone());
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
        self.fsm.lock().unwrap().mv(Event::CONNECTION_TCP_CR_ACKED);
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
