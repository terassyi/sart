use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::stream::{FuturesUnordered, Next};
use futures::{FutureExt, SinkExt, Stream, StreamExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::Notify;
use tokio::time::{interval_at, sleep, Instant, Interval, Sleep};
use tokio_util::codec::{Framed, FramedRead};

use crate::bgp::capability::{Capability, CapabilitySet};
use crate::bgp::config::NeighborConfig;
use crate::bgp::error::{
    Error, MessageHeaderError, OpenMessageError, PeerError, UpdateMessageError,
};
use crate::bgp::event::{
    AdministrativeEvent, BgpMessageEvent, Event, TcpConnectionEvent, TimerEvent,
};
use crate::bgp::packet::codec::Codec;
use crate::bgp::packet::message::{
    Message, MessageBuilder, MessageType, NotificationCode, NotificationSubCode,
};
use crate::bgp::server::Bgp;

use super::fsm::State;
use super::{fsm::FiniteStateMachine, neighbor::Neighbor};

#[derive(Debug)]
pub(crate) struct PeerManager {
    pub info: Arc<Mutex<PeerInfo>>,
    pub event_tx: UnboundedSender<Event>,
}

impl PeerManager {
    pub fn new(config: Arc<Mutex<PeerInfo>>, event_tx: UnboundedSender<Event>) -> Self {
        Self { info: config, event_tx }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PeerInfo {
    pub neighbor: Neighbor,
    pub asn: u32,
    pub router_id: Ipv4Addr,
    pub hold_time: u64,
    pub keepalive_time: u64,
    pub connect_retry_time: u64,
    pub local_capabilities: CapabilitySet,
    pub state: State,
}

impl PeerInfo {
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
            state: State::Idle,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Peer {
    info: Arc<Mutex<PeerInfo>>,
    fsm: Mutex<FiniteStateMachine>,
    admin_rx: UnboundedReceiver<Event>,
    msg_tx: Option<UnboundedSender<Message>>,
    keepalive_timer: Interval,
    keepalive_time: u64,
    hold_timer: Timer,
    uptime: Instant,
    negotiated_hold_time: u64,
    connect_retry_counter: u32,
    drop_signal: Arc<Notify>,
    send_counter: Arc<Mutex<MessageCounter>>,
    recv_counter: Arc<Mutex<MessageCounter>>,
}

impl Peer {
    pub fn new(config: Arc<Mutex<PeerInfo>>, rx: UnboundedReceiver<Event>) -> Self {
        let keepalive_time = { config.lock().unwrap().keepalive_time };
        Self {
            info: config,
            fsm: Mutex::new(FiniteStateMachine::new()),
            admin_rx: rx,
            msg_tx: None,
            hold_timer: Timer::new(),
            keepalive_timer: interval_at(
                Instant::now() + Duration::new(u32::MAX.into(), 0),
                Duration::from_secs(keepalive_time),
            ),
            keepalive_time,
            uptime: Instant::now(),
            negotiated_hold_time: Bgp::DEFAULT_HOLD_TIME,
            connect_retry_counter: 0,
            drop_signal: Arc::new(Notify::new()),
            send_counter: Arc::new(Mutex::new(MessageCounter::new())),
            recv_counter: Arc::new(Mutex::new(MessageCounter::new())),
        }
    }

    fn state(&self) -> State {
        self.fsm.lock().unwrap().get_state()
    }

    fn move_state(&mut self, event: u8) {
        let state = self.fsm.lock().unwrap().mv(event);
        self.info.lock().unwrap().state = state;
    }

    #[tracing::instrument(skip(self))]
    pub async fn handle(&mut self) {
        // handle event
        let (msg_event_tx, mut msg_event_rx) = unbounded_channel::<BgpMessageEvent>();
        let (peer_addr, peer_as) = {
            let c = self.info.lock().unwrap();
            (c.neighbor.get_addr().to_string(), c.neighbor.get_asn())
        };

        loop {
            futures::select_biased! {
                // timer handling
                _ = self.hold_timer.ticker.next() => {
                    if self.hold_timer.last.elapsed().as_secs() >= self.hold_timer.interval {
                        let current_state = self.state();
                        tracing::warn!(peer.addr=peer_addr, peer.asn=peer_as, state=?current_state, event=%TimerEvent::HoldTimerExpire);
                        self.hold_timer_expire().unwrap();
                    }
                }
                _ = self.keepalive_timer.tick().fuse() => {
                    let current_state = self.state();
                    tracing::debug!(peer.addr=peer_addr, peer.asn=peer_as, state=?current_state, event=%TimerEvent::KeepaliveTimerExpire);
                    self.keepalive_timer_expire().unwrap();
                }
                // event handling
                event = self.admin_rx.recv().fuse() => {
                    if let Some(event) = event {
                        let current_state = self.state();
                        tracing::info!(peer.addr=peer_addr, peer.asn=peer_as, state=?current_state, event=%event);
                        match event {
                            Event::Admin(event) => match event {
                                AdministrativeEvent::ManualStart => self.manual_start().unwrap(),
                                _ => unimplemented!(),
                            },
                            Event::Connection(event) => match event {
                                TcpConnectionEvent::TcpCRAcked(stream) => {
                                    self.handle_tcp_connect(stream, msg_event_tx.clone(), Event::CONNECTION_TCP_CR_ACKED).unwrap();
                                },
                                TcpConnectionEvent::TcpConnectionConfirmed(stream) => {
                                    self.handle_tcp_connect(stream, msg_event_tx.clone(), Event::CONNECTION_TCP_CONNECTION_CONFIRMED).unwrap();
                                },
                                TcpConnectionEvent::TcpConnectionFail => self.tcp_connection_fail().unwrap(),
                                _ => unimplemented!(),
                            },
                            Event::Timer(event) => match event {
                                TimerEvent::ConnectRetryTimerExpire => self.connect_retry_timer_expire().unwrap(),
                                TimerEvent::HoldTimerExpire => {}, // don't handle here
                                TimerEvent::KeepaliveTimerExpire => {}, // don't handle here
                                _ => unimplemented!(),
                            },
                            _ => panic!("unhandlable event"),
                        }
                    }
                }
                event = msg_event_rx.recv().fuse() => {
                    if let Some(event) = event {
                        let current_state = self.state();
                        tracing::info!(peer.addr=peer_addr, peer.asn=peer_as, state=?current_state, event=%event);
                        match event {
                            BgpMessageEvent::BgpOpen(msg) => self.bgp_open(msg).unwrap(),
                            BgpMessageEvent::BgpHeaderError(e) => self.bgp_header_error(e).unwrap(),
                            BgpMessageEvent::BgpOpenMsgErr(e) => self.bgp_open_msg_error(e).unwrap(),
                            BgpMessageEvent::NotifMsgVerErr => self.notification_msg_ver_error().unwrap(),
                            BgpMessageEvent::NotifMsg(msg) => self.notification_msg(msg).unwrap(),
                            BgpMessageEvent::KeepAliveMsg => self.keepalive_msg().unwrap(),
                            BgpMessageEvent::UpdateMsg(msg) => self.update_msg().unwrap(),
                            BgpMessageEvent::UpdateMsgErr(e) => self.update_msg_error(e).unwrap(),
                            BgpMessageEvent::RouteRefreshMsg(msg) => self.route_refresh_msg().unwrap(),
                            BgpMessageEvent::RouteRefreshMsgErr => self.route_refresh_msg_error().unwrap(),
                            _ => unimplemented!(),
                        };
                    }
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

    #[tracing::instrument(skip(self, stream, msg_event_tx))]
    fn initialize(
        &mut self,
        stream: TcpStream,
        msg_event_tx: UnboundedSender<BgpMessageEvent>,
    ) -> Result<(), Error> {
        let codec = Framed::new(stream, Codec::new(true, false));
        let (msg_tx, mut msg_rx) = unbounded_channel::<Message>();
        self.msg_tx = Some(msg_tx);
        let drop_signal = self.drop_signal.clone();
        let (peer_addr, peer_as) = {
            let c = self.info.lock().unwrap();
            (c.neighbor.get_addr().to_string(), c.neighbor.get_asn())
        };
        let send_counter = self.send_counter.clone();
        let recv_counter = self.recv_counter.clone();
        tokio::spawn(async move {
            let (mut sender, mut receiver) = codec.split();
            loop {
                futures::select_biased! {
                    msg = receiver.next().fuse() => {
                        if let Some(msg_result) = msg {
                            match msg_result {
                                Ok(msg) => {
                                    match msg.msg_type() {
                                        MessageType::Open => {
                                            recv_counter.lock().unwrap().open += 1;
                                            msg_event_tx.send(BgpMessageEvent::BgpOpen(msg)).unwrap();
                                        },
                                        MessageType::Update => {
                                            recv_counter.lock().unwrap().update += 1;
                                            msg_event_tx.send(BgpMessageEvent::UpdateMsg(msg)).unwrap();
                                        },
                                        MessageType::Keepalive => {
                                            recv_counter.lock().unwrap().keepalive += 1;
                                            msg_event_tx.send(BgpMessageEvent::KeepAliveMsg).unwrap();
                                        },
                                        MessageType::Notification => {
                                            recv_counter.lock().unwrap().notification += 1;
                                            msg_event_tx.send(BgpMessageEvent::NotifMsg(msg)).unwrap();
                                        },
                                        MessageType::RouteRefresh => {
                                            recv_counter.lock().unwrap().route_refresh += 1;
                                            msg_event_tx.send(BgpMessageEvent::RouteRefreshMsg(msg)).unwrap();
                                        },
                                    };
                                },
                                Err(e) => {
                                    match e {
                                        Error::MessageHeader(e) => {
                                            tracing::error!("{:?}", e);
                                            msg_event_tx.send(BgpMessageEvent::BgpHeaderError(e)).unwrap();
                                        },
                                        Error::OpenMessage(e) => {
                                            tracing::error!("{:?}", e);
                                            msg_event_tx.send(BgpMessageEvent::BgpOpenMsgErr(e)).unwrap();
                                        },
                                        Error::UpdateMessage(e) => {
                                            tracing::error!("{:?}", e);
                                            msg_event_tx.send(BgpMessageEvent::UpdateMsgErr(e)).unwrap();
                                        },
                                        _ => tracing::error!("{:?}", e),
                                    }
                                },
                            }
                        }
                    }
                    msg = msg_rx.recv().fuse() => {
                        if let Some(msg) = msg {
                            match msg.msg_type() {
                                MessageType::Open => send_counter.lock().unwrap().open += 1,
                                MessageType::Update => send_counter.lock().unwrap().update += 1,
                                MessageType::Keepalive => send_counter.lock().unwrap().keepalive += 1,
                                MessageType::Notification => send_counter.lock().unwrap().notification += 1,
                                MessageType::RouteRefresh => send_counter.lock().unwrap().route_refresh += 1,
                            };
                            sender.send(msg).await;
                        }
                    }
                    _ = drop_signal.notified().fuse() => {
                        tracing::warn!(peer.addr=peer_addr, peer.asn=peer_as, "drop connection");
                        return;
                    }
                }
            }
        });
        Ok(())
    }

    fn release(&mut self) -> Result<(), Error> {
        self.keepalive_time = 0;
        self.keepalive_timer = interval_at(Instant::now() + Duration::new(u32::MAX.into(), 0), Duration::new(u32::MAX.into(), 0));
        self.hold_timer.stop();
        self.negotiated_hold_time = Bgp::DEFAULT_HOLD_TIME;
        self.msg_tx = None;
        self.send_counter.lock().unwrap().reset();
        self.recv_counter.lock().unwrap().reset();
        Ok(())
    }

    fn drop_connection(&self) {
        self.drop_signal.notify_one();
    }

    // Event 1
    fn manual_start(&mut self) -> Result<(), Error> {
        self.move_state(Event::AMDIN_MANUAL_START);
        Ok(())
    }

    // Event 2
    fn manual_stop(&mut self) -> Result<(), Error> {
        self.move_state(Event::ADMIN_MANUAL_STOP);
        Ok(())
    }

    // Event 9
    fn connect_retry_timer_expire(&mut self) -> Result<(), Error> {
        match self.state() {
            State::Idle => {}
            State::Connect => {
                // drops the TCP connection,
                // restarts the ConnectRetryTimer,
                // stops the DelayOpenTimer and resets the timer to zero,
                // initiates a TCP connection to the other BGP peer,
                // continues to listen for a connection that may be initiated by the remote BGP peer, and
                // stays in the Connect state.

                // nothing to do here
            }
            State::Active => {
                // restarts the ConnectRetryTimer (with initial value),
                // initiates a TCP connection to the other BGP peer,
                // continues to listen for a TCP connection that may be initiated by a remote BGP peer, and
                // changes its state to Connect.

                // nothing to do here
            }
            _ => {
                // sends the NOTIFICATION with the Error Code Finite State Machine Error,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                let msg = builder
                    .code(NotificationCode::FiniteStateMachine)?
                    .build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
        }
        self.fsm
            .lock()
            .unwrap()
            .mv(Event::TIMER_CONNECT_RETRY_TIMER_EXPIRE);
        Ok(())
    }

    // Event 10
    fn hold_timer_expire(&mut self) -> Result<(), Error> {
        match self.state() {
            State::OpenSent | State::OpenConfirm | State::Established => {
                // sends a Notification message with the error code Hold Timer Expired
                // sets the ConnectRetryTimer to zero
                // releases all BGP resources
                // drops the tcp connection
                // increments the ConnectRetryCounter
                // changes its state to Idle
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                let msg = builder.code(NotificationCode::HoldTimerExpired)?.build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            _ => {
                // releases all BGP resources
                // drops the tcp connection
                // increments ConnectRetryCounter by 1
                // stay
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
        }
        self.move_state(Event::TIMER_HOLD_TIMER_EXPIRE);
        Ok(())
    }

    // Event 11
    fn keepalive_timer_expire(&mut self) -> Result<(), Error> {
        match self.state() {
            State::Established => {
                // sends a KEEPALIVE message
                // restarts its KeepaliveTimer, unless the negotiated HoldTime value is zero.
                // Each time the local system sends a KEEPALIVE or UPDATE message, it restarts its KeepaliveTimer, unless the negotiated HoldTime value is zero.
                if self.keepalive_time == 0 {
                    return Ok(())
                }
                let msg = Self::build_keepalive_msg()?;
                self.send(msg)?;
            }
            State::OpenSent => {
                // sends the NOTIFICATION with the Error Code Finite State Machine Error,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                let msg = builder
                    .code(NotificationCode::FiniteStateMachine)?
                    .build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            State::OpenConfirm => {
                // sends a KEEPALIVE message,
                // restarts the KeepaliveTimer, and
                // remains in the OpenConfirmed state
                let msg = Self::build_keepalive_msg()?;
                self.send(msg)?;
            }
            _ => {
                // if the ConnectRetryTimer is running, stops and resets the ConnectRetryTimer (sets to zero),
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
        }
        self.fsm
            .lock()
            .unwrap()
            .mv(Event::TIMER_KEEPALIVE_TIMER_EXPIRE);
        Ok(())
    }

    // tcp_cr_acked and tcp_connection_confirmed
    // Event 16, 17
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
                let hold_time = { self.info.lock().unwrap().hold_time };
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
                let hold_time = { self.info.lock().unwrap().hold_time };
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
        self.move_state(event);
        Ok(())
    }

    // Event 18
    fn tcp_connection_fail(&mut self) -> Result<(), Error> {
        match self.state() {
            State::Idle => {}
            State::Connect | State::Active | State::OpenConfirm => {
                // stops the ConnectRetryTimer to zero,
                // drops the TCP connection,
                // releases all BGP resources, and
                // changes its state to Idle.
                self.drop_connection();
                self.release()?;
            }
            State::OpenSent => {
                // closes the BGP connection,
                // restarts the ConnectRetryTimer,
                // continues to listen for a connection that may be initiated by the remote BGP peer, and
                // changes its state to Active.

                // nothing to do here
            }
            State::Established => {
                // sets the ConnectRetryTimer to zero,
                // deletes all routes associated with this connection,
                // releases all the BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                self.drop_connection();
                self.release()?;
                self.connect_retry_counter += 1;
            }
        }
        self.fsm
            .lock()
            .unwrap()
            .mv(Event::CONNECTION_TCP_CONNECTION_FAIL);
        Ok(())
    }

    // Event 19
    fn bgp_open(&mut self, recv: Message) -> Result<(), Error> {
        let (_version, _asn, hold_time, _router_id, _caps) = recv.to_open()?;
        match self.state() {
            State::Active => {
                // stops the ConnectRetryTimer (if running) and sets the ConnectRetryTimer to zero,
                // stops and clears the DelayOpenTimer (sets to zero),
                // completes the BGP initialization,
                // sends an OPEN message,
                // sends a KEEPALIVE message,
                // if the HoldTimer value is non-zero,
                //   - starts the KeepaliveTimer to initial value,
                //   - resets the HoldTimer to the negotiated value,
                // else if the HoldTimer is zero
                //   - resets the KeepaliveTimer (set to zero),
                //   - resets the HoldTimer to zero, and
                // changes its state to OpenConfirm.
                let msg = self.build_open_msg()?;
                self.send(msg)?;
                let msg = Self::build_keepalive_msg()?;
                self.send(msg)?;
                let _negotiated_hold_time = self.negotiate_hold_time(hold_time as u64)?;
            }
            State::OpenSent => {
                // Collision detection mechanisms (Section 6.8) need to be applied
                // sets the BGP ConnectRetryTimer to zero,
                // sends a KEEPALIVE message, and
                // sets a KeepaliveTimer (via the text below)
                // sets the HoldTimer according to the negotiated value (see Section 4.2),
                // changes its state to OpenConfirm
                let msg = Self::build_keepalive_msg()?;
                let _negotiated_hold_time = self.negotiate_hold_time(hold_time as u64)?;
                self.send(msg)?;
            }
            State::Established => {
                // sends a NOTIFICATION with a Cease,
                // sets the ConnectRetryTimer to zero,
                // deletes all routes associated with this connection,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                let msg = builder.code(NotificationCode::Cease)?.build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            _ => {}
        }
        self.move_state(Event::MESSAGE_BGP_OPEN);
        Ok(())
    }

    // Event 21
    fn bgp_header_error(&mut self, e: MessageHeaderError) -> Result<(), Error> {
        match self.state() {
            State::Active | State::Connect => {
                // stops the ConnectRetryTimer (if running) and sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            State::OpenSent | State::OpenConfirm => {
                // sends a NOTIFICATION message with the appropriate error code,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                builder.code(NotificationCode::MessageHeader)?;
                if let Some(subcode) = NotificationSubCode::try_from_with_code(
                    e.into(),
                    NotificationCode::MessageHeader,
                )? {
                    builder.subcode(subcode)?;
                }
                let msg = builder.build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            State::Established => {
                // sends a NOTIFICATION message with the Error Code Finite State Machine Error,
                // deletes all routes associated with this connection,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                builder.code(NotificationCode::MessageHeader)?;
                if let Some(subcode) = NotificationSubCode::try_from_with_code(
                    e.into(),
                    NotificationCode::MessageHeader,
                )? {
                    builder.subcode(subcode)?;
                }
                let msg = builder.build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            _ => {}
        }
        self.move_state(Event::MESSAGE_BGP_HEADER_ERROR);
        Ok(())
    }

    // Event 22
    fn bgp_open_msg_error(&mut self, e: OpenMessageError) -> Result<(), Error> {
        match self.state() {
            State::Active | State::Connect => {
                // stops the ConnectRetryTimer (if running) and sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            State::OpenSent | State::OpenConfirm => {
                // sends a NOTIFICATION message with the appropriate error code,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                builder.code(NotificationCode::OpenMessage)?;
                if let Some(subcode) = NotificationSubCode::try_from_with_code(
                    e.into(),
                    NotificationCode::MessageHeader,
                )? {
                    builder.subcode(subcode)?;
                }
                let msg = builder.build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            State::Established => {
                // sends a NOTIFICATION message with the Error Code Finite State Machine Error,
                // deletes all routes associated with this connection,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                let msg = builder
                    .code(NotificationCode::FiniteStateMachine)?
                    .build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            _ => {}
        }
        self.fsm
            .lock()
            .unwrap()
            .mv(Event::MESSAGE_BGP_OPEN_MSG_ERROR);
        Ok(())
    }

    // Event 24
    fn notification_msg_ver_error(&mut self) -> Result<(), Error> {
        match self.state() {
            State::Active | State::Connect => {
                // stops the ConnectRetryTimer (if running) and sets the ConnectRetryTimer to zero,
                // stops and resets the DelayOpenTimer (sets to zero),
                // releases all BGP resources,
                // drops the TCP connection, and
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
            }
            State::OpenConfirm | State::OpenSent => {
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection, and
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
            }
            State::Established => {
                // sets the ConnectRetryTimer to zero,
                // deletes all routes associated with this connection,
                // releases all the BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            _ => {}
        }
        self.move_state(Event::MESSAGE_NOTIF_MSG_ERROR);
        Ok(())
    }

    // Event 25
    #[tracing::instrument(skip(self, msg))]
    fn notification_msg(&mut self, msg: Message) -> Result<(), Error> {
        let (code, subcode, data) = msg.to_notification()?;
        let (peer_addr, peer_as) = {
            let conf = self.info.lock().unwrap();
            (conf.neighbor.get_addr().to_string(), conf.neighbor.get_asn())
        };
        tracing::warn!(peer.addr=peer_addr, peer.asn=peer_as, notification_code=?code, notification_subcode=?subcode);
        match self.state() {
            State::OpenSent => {
                // sends the NOTIFICATION with the Error Code Finite State Machine Error,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                let msg = builder
                    .code(NotificationCode::FiniteStateMachine)?
                    .build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            State::Established => {
                // sets the ConnectRetryTimer to zero,
                // deletes all routes associated with this connection,
                // releases all the BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            _ => {
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
        }
        self.move_state(Event::MESSAGE_NOTIF_MSG);
        Ok(())
    }

    // Event 26
    fn keepalive_msg(&mut self) -> Result<(), Error> {
        match self.state() {
            State::OpenSent => {
                // sends the NOTIFICATION with the Error Code Finite State Machine Error,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                let msg = builder
                    .code(NotificationCode::FiniteStateMachine)?
                    .build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            State::OpenConfirm | State::Established => {
                // restarts the HoldTimer and if the negotiated HoldTime value is non-zero, and
                // changes(remains) its state to Established.
                self.hold_timer.push(self.negotiated_hold_time);
            }
            _ => {
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by one,
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
        }
        self.move_state(Event::MESSAGE_KEEPALIVE_MSG);
        Ok(())
    }

    // Event 27
    fn update_msg(&mut self) -> Result<(), Error> {
        match self.state() {
            State::OpenSent | State::OpenConfirm => {
                // sends a NOTIFICATION with a code of Finite State Machine Error,
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                let msg = builder
                    .code(NotificationCode::FiniteStateMachine)?
                    .build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            State::Established => {
                // processes the message,
                // restarts its HoldTimer, if the negotiated HoldTime value is non-zero, and
                // remains in the Established state.
            }
            _ => {
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by one,
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
        }
        self.move_state(Event::MESSAGE_UPDATE_MSG);
        Ok(())
    }

    // Event 28
    fn update_msg_error(&mut self, e: UpdateMessageError) -> Result<(), Error> {
        match self.state() {
            State::OpenSent | State::OpenConfirm => {
                // sends a NOTIFICATION with a code of Finite State Machine Error,
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                let msg = builder
                    .code(NotificationCode::FiniteStateMachine)?
                    .build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            State::Established => {
                // sends a NOTIFICATION message with an Update error,
                // sets the ConnectRetryTimer to zero,
                // deletes all routes associated with this connection,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
                let mut builder = MessageBuilder::builder(MessageType::Notification);
                builder.code(NotificationCode::UpdateMessage)?;
                if let Some(subcode) = NotificationSubCode::try_from_with_code(
                    e.into(),
                    NotificationCode::MessageHeader,
                )? {
                    builder.subcode(subcode)?;
                }
                let msg = builder.build()?;
                self.send(msg)?;
                self.release()?;
                self.drop_connection();
                self.connect_retry_counter += 1;
            }
            _ => {
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by one,
                // changes its state to Idle.
                self.release()?;
                self.drop_connection();
            }
        }
        self.move_state(Event::MESSAGE_UPDATE_MSG_ERROR);
        Ok(())
    }

    fn route_refresh_msg(&self) -> Result<(), Error> {
        Ok(())
    }

    fn route_refresh_msg_error(&self) -> Result<(), Error> {
        Ok(())
    }

    #[tracing::instrument(skip(self, received_hold_time))]
    fn negotiate_hold_time(&mut self, received_hold_time: u64) -> Result<u64, Error> {
        // This 2-octet unsigned integer indicates the number of seconds
        // the sender proposes for the value of the Hold Timer.  Upon
        // receipt of an OPEN message, a BGP speaker MUST calculate the
        // value of the Hold Timer by using the smaller of its configured
        // Hold Time and the Hold Time received in the OPEN message.  The
        // Hold Time MUST be either zero or at least three seconds.  An
        // implementation MAY reject connections on the basis of the Hold
        // Time.  The calculated value indicates the maximum number of
        // seconds that may elapse between the receipt of successive
        // KEEPALIVE and/or UPDATE messages from the sender.
        let local_hold_time = self.info.lock().unwrap().hold_time;
        let negotiated_hold_time = if local_hold_time > received_hold_time {
            received_hold_time
        } else {
            local_hold_time
        };
        if negotiated_hold_time > 0 && negotiated_hold_time < 3 {
            return Err(Error::OpenMessage(OpenMessageError::UnacceptableHoldTime));
        }
        if negotiated_hold_time != 0 {
            self.negotiated_hold_time = negotiated_hold_time;
            self.hold_timer.push(negotiated_hold_time);
            self.keepalive_time = negotiated_hold_time / 3;
            self.keepalive_timer =
                interval_at(Instant::now(), Duration::new(negotiated_hold_time / 3, 0));
        } else {
            self.hold_timer.stop();
            self.keepalive_time = 0;
            self.keepalive_timer =
                interval_at(Instant::now(), Duration::new(u64::MAX, 0));
        }
        let (peer_addr, peer_as) = {
            let conf = self.info.lock().unwrap();
            (conf.neighbor.get_addr().to_string(), conf.neighbor.get_asn())
        };
        tracing::info!(peer.addr=peer_addr, peer.asn=peer_as, hold_time=negotiated_hold_time, keepalive_time=self.keepalive_time);

        Ok(negotiated_hold_time)
    }

    fn build_open_msg(&self) -> Result<Message, Error> {
        let conf = self.info.lock().unwrap();
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

    fn build_keepalive_msg() -> Result<Message, Error> {
        let builder = MessageBuilder::builder(MessageType::Keepalive);
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
        ticker.push(sleep(Duration::new(u32::MAX.into(), 0)));
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

    fn stop(&mut self) {
        self.interval = 0;
        self.last = Instant::now();
    }
}

#[derive(Debug, Clone, Copy)]
struct MessageCounter {
    open: usize,
    update: usize,
    keepalive: usize,
    notification: usize,
    route_refresh: usize,
}

impl MessageCounter {
    fn new() -> Self {
        Self {
            open: 0,
            update: 0,
            keepalive: 0, 
            notification: 0,
            route_refresh: 0,
        }
    }

    fn sum(&self) -> usize {
        self.open + self.update + self.keepalive + self.notification + self.route_refresh
    }

    fn reset(&mut self) {
        self.open = 0;
        self.update = 0;
        self.keepalive = 0;
        self.notification = 0;
        self.route_refresh = 0;
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
    use crate::bgp::event::Event;
    use crate::bgp::peer::fsm::State;

    use super::{Peer, PeerInfo};
    use rstest::rstest;

    #[tokio::test]
    async fn works_peer_move_event() {
        let event = vec![Event::AMDIN_MANUAL_START, Event::TIMER_CONNECT_RETRY_TIMER_EXPIRE, Event::CONNECTION_TCP_CONNECTION_CONFIRMED, Event::MESSAGE_BGP_OPEN, Event::MESSAGE_KEEPALIVE_MSG];
        let info = Arc::new(Mutex::new(PeerInfo::new(
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
        let mut peer = Peer::new(info, rx);
        for e in event.into_iter() {
            peer.move_state(e);
        }
        assert_eq!(State::Established, peer.info.lock().unwrap().state);
    }

    #[tokio::test]
    async fn works_peer_build_open_msg() {
        let info = Arc::new(Mutex::new(PeerInfo::new(
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
        let peer = Peer::new(info, rx);
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
