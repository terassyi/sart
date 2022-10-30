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
use crate::bgp::error::{Error, PeerError};
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
    keepalive_timer: Interval,
    hold_timer: Timer,
    uptime: Instant,
    negotiated_hold_time: u64,
    connect_retry_counter: u32,
    drop_signal: Arc<Notify>,
}

impl Peer {
    pub fn new(config: Arc<Mutex<PeerConfig>>, rx: UnboundedReceiver<Event>) -> Self {
        let keepalive_time = { config.lock().unwrap().keepalive_time };
        Self {
            config,
            fsm: Mutex::new(FiniteStateMachine::new()),
            admin_rx: rx,
            msg_tx: None,
            hold_timer: Timer::new(),
            keepalive_timer: interval_at(
                Instant::now() + Duration::new(u32::MAX.into(), 0),
                Duration::from_secs(keepalive_time),
            ),
            uptime: Instant::now(),
            negotiated_hold_time: Bgp::DEFAULT_HOLD_TIME,
            connect_retry_counter: 0,
            drop_signal: Arc::new(Notify::new()),
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
                                AdministrativeEvent::ManualStart => self.manual_start().unwrap(),
                                _ => unimplemented!(),
                            },
                            Event::Connection(event) => match event {
                                TcpConnectionEvent::TcpCRAcked(stream) => {
                                    println!("  {:?}", stream.peer_addr());
                                    self.handle_tcp_connect(stream, msg_event_tx.clone(), Event::CONNECTION_TCP_CR_ACKED).unwrap();
                                },
                                TcpConnectionEvent::TcpConnectionConfirmed(stream) => {
                                    println!("  {:?}", stream.peer_addr());
                                    self.handle_tcp_connect(stream, msg_event_tx.clone(), Event::CONNECTION_TCP_CONNECTION_CONFIRMED).unwrap();
                                },
                                TcpConnectionEvent::TcpConnectionFail => self.tcp_connection_fail().unwrap(),
                                _ => unimplemented!(),
                            },
                            Event::Timer(event) => match event {
                                TimerEvent::ConnectRetryTimerExpire => self.connect_retry_timer_expire().unwrap(),
                                TimerEvent::HoldTimerExpire => self.hold_timer_expire().unwrap(),
                                TimerEvent::KeepaliveTimerExpire => self.keepalive_timer_expire().unwrap(),
                                _ => unimplemented!(),
                            },
                            _ => panic!("unhandlable event"),
                        }
                    }
                }
                event = msg_event_rx.recv().fuse() => {
                    if let Some(event) = event {
                        match event {
                            BgpMessageEvent::BgpOpen(msg) => self.bgp_open().unwrap(),
                            BgpMessageEvent::BgpHeaderError => self.bgp_header_error().unwrap(),
                            BgpMessageEvent::BgpOpenMsgErr => self.bgp_open_msg_error().unwrap(),
                            BgpMessageEvent::NotifMsgVerErr => self.notification_msg_ver_error().unwrap(),
                            BgpMessageEvent::NotifMsg(msg) => self.notification_msg().unwrap(),
                            BgpMessageEvent::KeepAliveMsg => self.keepalive_msg().unwrap(),
                            BgpMessageEvent::UpdateMsg(msg) => self.update_msg().unwrap(),
                            BgpMessageEvent::UpdateMsgErr => self.update_msg_error().unwrap(),
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

    fn initialize(
        &mut self,
        stream: TcpStream,
        msg_event_tx: UnboundedSender<BgpMessageEvent>,
    ) -> Result<(), Error> {
        let mut codec = Framed::new(stream, Codec::new(true, false));
        let (msg_tx, mut msg_rx) = unbounded_channel::<Message>();
        self.msg_tx = Some(msg_tx);
        let drop_signal = self.drop_signal.clone();
        tokio::spawn(async move {
            println!("message handler process is running");
            let (mut sender, mut receiver) = codec.split();
            loop {
                futures::select_biased! {
                    msg = receiver.next().fuse() => {
                        if let Some(msg_result) = msg {
                            match msg_result {
                                Ok(msg) => {
                                    println!("{:?}", msg);
                                    match msg.msg_type() {
                                        MessageType::Open => msg_event_tx.send(BgpMessageEvent::BgpOpen(msg)).unwrap(),
                                        MessageType::Update => msg_event_tx.send(BgpMessageEvent::UpdateMsg(msg)).unwrap(),
                                        MessageType::Keepalive => msg_event_tx.send(BgpMessageEvent::KeepAliveMsg).unwrap(),
                                        MessageType::Notification => msg_event_tx.send(BgpMessageEvent::NotifMsg(msg)).unwrap(),
                                        MessageType::RouteRefresh => msg_event_tx.send(BgpMessageEvent::RouteRefreshMsg(msg)).unwrap(),
                                    };
                                },
                                Err(e) => {
                                    match e {
                                        Error::MessageHeader(e) => {
                                            println!("{:?}", e);
                                            msg_event_tx.send(BgpMessageEvent::BgpHeaderError).unwrap();
                                        },
                                        Error::OpenMessage(e) => {
                                            println!("{:?}", e);
                                            msg_event_tx.send(BgpMessageEvent::BgpOpenMsgErr).unwrap();
                                        },
                                        Error::UpdateMessage(e) => {
                                            println!("{:?}", e);
                                            msg_event_tx.send(BgpMessageEvent::UpdateMsgErr).unwrap();
                                        },
                                        _ => println!("{:?}", e),
                                    }
                                },
                            }
                        }
                    }
                    msg = msg_rx.recv().fuse() => {
                        if let Some(msg) = msg {
                            sender.send(msg).await;
                        }
                    }
                    _ = drop_signal.notified().fuse() => {

                    }
                }
            }
        });
        Ok(())
    }

    fn release(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn drop_connection(&self) {
        self.drop_signal.notify_one();
    }

    // Event 1
    fn manual_start(&mut self) -> Result<(), Error> {
        self.fsm.lock().unwrap().mv(Event::AMDIN_MANUAL_START);
        Ok(())
    }

    // Event 2
    fn manual_stop(&mut self) -> Result<(), Error> {
        self.fsm.lock().unwrap().mv(Event::ADMIN_MANUAL_STOP);
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
        self.fsm.lock().unwrap().mv(Event::TIMER_HOLD_TIMER_EXPIRE);
        Ok(())
    }

    // Event 11
    fn keepalive_timer_expire(&self) -> Result<(), Error> {
        match self.state() {
            State::Established => {
                // sends a KEEPALIVE message
                // restarts its KeepaliveTimer, unless the negotiated HoldTime value is zero.
                // Each time the local system sends a KEEPALIVE or UPDATE message, it restarts its KeepaliveTimer, unless the negotiated HoldTime value is zero.
            }
            State::OpenSent => {
                // sends the NOTIFICATION with the Error Code Finite State Machine Error,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
            }
            State::OpenConfirm => {
                // sends a KEEPALIVE message,
                // restarts the KeepaliveTimer, and
                // remains in the OpenConfirmed state
            }
            _ => {
                // if the ConnectRetryTimer is running, stops and resets the ConnectRetryTimer (sets to zero),
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
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

    // Event 18
    fn tcp_connection_fail(&self) -> Result<(), Error> {
        match self.state() {
            State::Idle => {}
            State::Connect | State::Active | State::OpenConfirm => {
                // stops the ConnectRetryTimer to zero,
                // drops the TCP connection,
                // releases all BGP resources, and
                // changes its state to Idle.
            }
            State::OpenSent => {
                // closes the BGP connection,
                // restarts the ConnectRetryTimer,
                // continues to listen for a connection that may be initiated by the remote BGP peer, and
                // changes its state to Active.
            }
            State::Established => {
                // sets the ConnectRetryTimer to zero,
                // deletes all routes associated with this connection,
                // releases all the BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
        }
        self.fsm
            .lock()
            .unwrap()
            .mv(Event::CONNECTION_TCP_CONNECTION_FAIL);
        Ok(())
    }

    // Event 19
    fn bgp_open(&self) -> Result<(), Error> {
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
            }
            State::OpenSent => {
                // Collision detection mechanisms (Section 6.8) need to be applied
                // sets the BGP ConnectRetryTimer to zero,
                // sends a KEEPALIVE message, and
                // sets a KeepaliveTimer (via the text below)
                // sets the HoldTimer according to the negotiated value (see Section 4.2),
                // changes its state to OpenConfirm
            }
            State::Established => {
                // sends a NOTIFICATION with a Cease,
                // sets the ConnectRetryTimer to zero,
                // deletes all routes associated with this connection,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
            _ => {}
        }
        self.fsm.lock().unwrap().mv(Event::MESSAGE_BGP_OPEN);
        Ok(())
    }

    // Event 21
    fn bgp_header_error(&self) -> Result<(), Error> {
        match self.state() {
            State::Active | State::Connect => {
                // stops the ConnectRetryTimer (if running) and sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
            State::OpenSent | State::OpenConfirm => {
                // sends a NOTIFICATION message with the appropriate error code,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
            State::Established => {
                // sends a NOTIFICATION message with the Error Code Finite State Machine Error,
                // deletes all routes associated with this connection,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
            _ => {}
        }
        self.fsm.lock().unwrap().mv(Event::MESSAGE_BGP_HEADER_ERROR);
        Ok(())
    }

    // Event 22
    fn bgp_open_msg_error(&self) -> Result<(), Error> {
        match self.state() {
            State::Active | State::Connect => {
                // stops the ConnectRetryTimer (if running) and sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
            State::OpenSent | State::OpenConfirm => {
                // sends a NOTIFICATION message with the appropriate error code,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
            State::Established => {
                // sends a NOTIFICATION message with the Error Code Finite State Machine Error,
                // deletes all routes associated with this connection,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
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
    fn notification_msg_ver_error(&self) -> Result<(), Error> {
        match self.state() {
            State::Active | State::Connect => {
                // stops the ConnectRetryTimer (if running) and sets the ConnectRetryTimer to zero,
                // stops and resets the DelayOpenTimer (sets to zero),
                // releases all BGP resources,
                // drops the TCP connection, and
                // changes its state to Idle.
            }
            State::OpenConfirm | State::OpenSent => {
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection, and
                // changes its state to Idle.
            }
            State::Established => {
                // sets the ConnectRetryTimer to zero,
                // deletes all routes associated with this connection,
                // releases all the BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
            _ => {}
        }
        self.fsm.lock().unwrap().mv(Event::MESSAGE_NOTIF_MSG_ERROR);
        Ok(())
    }

    // Event 25
    fn notification_msg(&self) -> Result<(), Error> {
        match self.state() {
            State::OpenSent => {
                // sends the NOTIFICATION with the Error Code Finite State Machine Error,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
            State::Established => {
                // sets the ConnectRetryTimer to zero,
                // deletes all routes associated with this connection,
                // releases all the BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
            _ => {
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
        }
        self.fsm.lock().unwrap().mv(Event::MESSAGE_NOTIF_MSG);
        Ok(())
    }

    // Event 26
    fn keepalive_msg(&self) -> Result<(), Error> {
        match self.state() {
            State::OpenSent => {
                // sends the NOTIFICATION with the Error Code Finite State Machine Error,
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
            }
            State::OpenConfirm | State::Established => {
                // restarts the HoldTimer and if the negotiated HoldTime value is non-zero, and
                // changes(remains) its state to Established.
            }
            _ => {
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by one,
                // changes its state to Idle.
            }
        }
        self.fsm.lock().unwrap().mv(Event::MESSAGE_KEEPALIVE_MSG);
        Ok(())
    }

    // Event 27
    fn update_msg(&self) -> Result<(), Error> {
        match self.state() {
            State::OpenSent | State::OpenConfirm => {
                // sends a NOTIFICATION with a code of Finite State Machine Error,
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
            }
        }
        self.fsm.lock().unwrap().mv(Event::MESSAGE_UPDATE_MSG);
        Ok(())
    }

    // Event 28
    fn update_msg_error(&self) -> Result<(), Error> {
        match self.state() {
            State::OpenSent | State::OpenConfirm => {
                // sends a NOTIFICATION with a code of Finite State Machine Error,
            }
            State::Established => {
                // sends a NOTIFICATION message with an Update error,
                // sets the ConnectRetryTimer to zero,
                // deletes all routes associated with this connection,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by 1,
                // changes its state to Idle.
            }
            _ => {
                // sets the ConnectRetryTimer to zero,
                // releases all BGP resources,
                // drops the TCP connection,
                // increments the ConnectRetryCounter by one,
                // changes its state to Idle.
            }
        }
        self.fsm.lock().unwrap().mv(Event::MESSAGE_UPDATE_MSG_ERROR);
        Ok(())
    }

    fn route_refresh_msg(&self) -> Result<(), Error> {
        Ok(())
    }

    fn route_refresh_msg_error(&self) -> Result<(), Error> {
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
