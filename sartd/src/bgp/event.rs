use tokio::net::TcpStream;

use crate::bgp::error::Error;
use std::convert::TryFrom;
use std::convert::TryInto;

use super::config::NeighborConfig;

#[derive(Debug)]
pub(crate) enum Event {
    Admin(AdministrativeEvent),
    Timer(TimerEvent),
    Connection(TcpConnectionEvent),
    Message(BgpMessageEvent),
    Control(ControlEvent),
}

impl Event {
    pub const AMDIN_MANUAL_START: u8 = 1;
    pub const ADMIN_MANUAL_STOP: u8 = 2;
    pub const ADMIN_AUTOMATIC_START: u8 = 3;
    pub const ADMIN_MANUAL_START_WITH_PASSIVE_TCP_ESTABLISHMENT: u8 = 4;
    pub const ADMIN_AUTOMATIC_START_WITH_PASSIVE_TCP_ESTABLISHMENT: u8 = 5;
    pub const ADMIN_AUTOMATIC_START_WITH_DAMP_PEER_OSCILLATIONS: u8 = 6;
    pub const ADMIN_AUTOMATIC_START_WITH_DAMP_PEER_OSCILLATIONS_AND_PASSIVE_TCP_ESTABLISHMENT: u8 =
        7;
    pub const ADMIN_AUTOMATIC_STOP: u8 = 8;
    pub const TIMER_CONNECT_RETRY_TIMER_EXPIRE: u8 = 9;
    pub const TIMER_HOLD_TIMER_EXPIRE: u8 = 10;
    pub const TIMER_KEEPALIVE_TIMER_EXPIRE: u8 = 11;
    pub const TIMER_DELAY_OPEN_TIMER_EXPIRE: u8 = 12;
    pub const TIMER_IDLE_HOLD_TIMER_EXPIRE: u8 = 13;
    pub const CONNECTION_TCP_CONNECTION_VALID: u8 = 14;
    pub const CONNECTION_TCP_CR_INVALID: u8 = 15;
    pub const CONNECTION_TCP_CR_ACKED: u8 = 16;
    pub const CONNECTION_TCP_CONNECTION_CONFIRMED: u8 = 17;
    pub const CONNECTION_TCP_CONNECTION_FAIL: u8 = 18;
    pub const MESSAGE_BGP_OPEN: u8 = 19;
    pub const MESSAGE_BGP_OPEN_WITH_DELAY_OPEN_TIMER_RUNNING: u8 = 20;
    pub const MESSAGE_BGP_HEADER_ERROR: u8 = 21;
    pub const MESSAGE_BGP_OPEN_MSG_ERROR: u8 = 22;
    pub const MESSAGE_OPEN_COLLISION_DUMP: u8 = 23;
    pub const MESSAGE_NOTIF_MSG_ERROR: u8 = 24;
    pub const MESSAGE_NOTIF_MSG: u8 = 25;
    pub const MESSAGE_KEEPALIVE_MSG: u8 = 26;
    pub const MESSAGE_UPDATE_MSG: u8 = 27;
    pub const MESSAGE_UPDATE_MSG_ERROR: u8 = 28;
}

impl Into<u8> for &Event {
    fn into(self) -> u8 {
        match *self {
			Event::Admin(AdministrativeEvent::ManualStart) => 1,
			Event::Admin(AdministrativeEvent::ManualStop) => 2,
			Event::Admin(AdministrativeEvent::AutomaticStart) => 3,
			Event::Admin(AdministrativeEvent::ManualStartWithPassiveTcpEstablishment) => 4,
			Event::Admin(AdministrativeEvent::AutomaticStartWithPassiveTcpEstablishment) => 5,
			Event::Admin(AdministrativeEvent::AutomaticStartWithDampPeerOscillations) => 6,
			Event::Admin(AdministrativeEvent::AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment) => 7,
			Event::Admin(AdministrativeEvent::AutomaticStop) => 8,
			Event::Timer(TimerEvent::ConnectRetryTimerExpire) => 9,
			Event::Timer(TimerEvent::HoldTimerExpire) => 10,
			Event::Timer(TimerEvent::KeepaliveTimerExpire) => 11,
			Event::Timer(TimerEvent::DelayOpenTimerExpire) => 12,
			Event::Timer(TimerEvent::IdleHoldTimerExpire) => 13,
			Event::Connection(TcpConnectionEvent::TcpConnectionValid) => 14,
			Event::Connection(TcpConnectionEvent::TcpCRInvalid) => 15,
			Event::Connection(TcpConnectionEvent::TcpCRAcked(_)) => 16,
			Event::Connection(TcpConnectionEvent::TcpConnectionConfirmed(_)) => 17,
			Event::Connection(TcpConnectionEvent::TcpConnectionFail) => 18,
			Event::Message(BgpMessageEvent::BgpOpen) => 19,
			Event::Message(BgpMessageEvent::BgpOpenWithDelayOpenTimerRunning) => 20,
			Event::Message(BgpMessageEvent::BgpHeaderError) => 21,
			Event::Message(BgpMessageEvent::BgpOpenMsgErr) => 22,
			Event::Message(BgpMessageEvent::OpenCollisionDump) => 23,
			Event::Message(BgpMessageEvent::NotifMsgVerErr) => 24,
			Event::Message(BgpMessageEvent::NotifMsg) => 25,
			Event::Message(BgpMessageEvent::KeepAliveMsg) => 26,
			Event::Message(BgpMessageEvent::UpdateMsg) => 27,
			Event::Message(BgpMessageEvent::UpdateMsgErr) => 28,
			_ => 0,
		}
    }
}

impl From<u8> for Event {
    fn from(_: u8) -> Self {
        Self::Admin(AdministrativeEvent::ManualStart)
    }
}

// https://www.rfc-editor.org/rfc/rfc4271#section-8.1.2
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AdministrativeEvent {
    ManualStart,
    ManualStop,
    #[allow(unused)]
    AutomaticStart,
    #[allow(unused)]
    ManualStartWithPassiveTcpEstablishment,
    #[allow(unused)]
    AutomaticStartWithPassiveTcpEstablishment,
    #[allow(unused)]
    AutomaticStartWithDampPeerOscillations,
    #[allow(unused)]
    AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment,
    #[allow(unused)]
    AutomaticStop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TimerEvent {
    ConnectRetryTimerExpire,
    HoldTimerExpire,
    KeepaliveTimerExpire,
    #[allow(unused)]
    DelayOpenTimerExpire,
    #[allow(unused)]
    IdleHoldTimerExpire,
}

#[derive(Debug)]
pub(crate) enum TcpConnectionEvent {
    #[allow(unused)]
    TcpConnectionValid,
    #[allow(unused)]
    TcpCRInvalid,
    TcpCRAcked(TcpStream),
    TcpConnectionConfirmed(TcpStream),
    TcpConnectionFail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum BgpMessageEvent {
    BgpOpen,
    #[allow(unused)]
    BgpOpenWithDelayOpenTimerRunning,
    BgpHeaderError,
    BgpOpenMsgErr,
    #[allow(unused)]
    OpenCollisionDump,
    NotifMsgVerErr,
    NotifMsg,
    KeepAliveMsg,
    UpdateMsg,
    UpdateMsgErr,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ControlEvent {
    AddPeer(NeighborConfig),
    Health,
}
