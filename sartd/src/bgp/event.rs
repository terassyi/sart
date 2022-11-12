use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;
use tokio::sync::mpsc::UnboundedSender;


use super::config::NeighborConfig;
use super::error::MessageHeaderError;
use super::error::OpenMessageError;
use super::error::UpdateMessageError;
use super::packet::message::Message;
use super::packet::prefix::Prefix;
use super::path::Path;
use super::peer::neighbor::NeighborPair;

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
			Event::Message(BgpMessageEvent::BgpOpen(_)) => 19,
			Event::Message(BgpMessageEvent::BgpOpenWithDelayOpenTimerRunning) => 20,
			Event::Message(BgpMessageEvent::BgpHeaderError(_)) => 21,
			Event::Message(BgpMessageEvent::BgpOpenMsgErr(_)) => 22,
			Event::Message(BgpMessageEvent::OpenCollisionDump) => 23,
			Event::Message(BgpMessageEvent::NotifMsgVerErr) => 24,
			Event::Message(BgpMessageEvent::NotifMsg(_)) => 25,
			Event::Message(BgpMessageEvent::KeepAliveMsg) => 26,
			Event::Message(BgpMessageEvent::UpdateMsg(_)) => 27,
			Event::Message(BgpMessageEvent::UpdateMsgErr(_)) => 28,
            Event::Message(BgpMessageEvent::RouteRefreshMsg(_)) => 100,
            Event::Message(BgpMessageEvent::RouteRefreshMsgErr) => 101,
			_ => 0,
		}
    }
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::Admin(e) => write!(f, "{}", e),
            Event::Timer(e) => write!(f, "{}", e),
            Event::Connection(e) => write!(f, "{}", e),
            Event::Message(e) => write!(f, "{}", e),
            Event::Control(e) => write!(f, "{}", e),
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

impl std::fmt::Display for AdministrativeEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			AdministrativeEvent::ManualStart => write!(f, "Admin_ManualStart"),
			AdministrativeEvent::ManualStop => write!(f, "Admin_ManualStop"),
			AdministrativeEvent::AutomaticStart => write!(f, "Admin_AutomaticStart"),
			AdministrativeEvent::ManualStartWithPassiveTcpEstablishment => write!(f, "Admin_ManualStartWithPassiveTcpEstablishment"),
			AdministrativeEvent::AutomaticStartWithPassiveTcpEstablishment => write!(f, "Admin_AutomaticStartWithPassiveTcpEstablishment"),
			AdministrativeEvent::AutomaticStartWithDampPeerOscillations => write!(f, "Admin_AutomaticStartWithDampPeerOscillations"),
			AdministrativeEvent::AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment => write!(f, "Admin_AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment"),
			AdministrativeEvent::AutomaticStop => write!(f, "Admin_AutomaticStop"),
        }
    }
}

impl Into<u8> for AdministrativeEvent {
    fn into(self) -> u8 {
        match self {
			AdministrativeEvent::ManualStart => 1,
			AdministrativeEvent::ManualStop => 2,
			AdministrativeEvent::AutomaticStart => 3,
			AdministrativeEvent::ManualStartWithPassiveTcpEstablishment => 4,
			AdministrativeEvent::AutomaticStartWithPassiveTcpEstablishment => 5,
			AdministrativeEvent::AutomaticStartWithDampPeerOscillations => 6,
			AdministrativeEvent::AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment => 7,
			AdministrativeEvent::AutomaticStop => 8,
        }
    }
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

impl std::fmt::Display for TimerEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimerEvent::ConnectRetryTimerExpire => write!(f, "Timer_ConnectRetryTimerExpire"),
            TimerEvent::HoldTimerExpire => write!(f, "Timer_HoldTimerExpire"),
            TimerEvent::KeepaliveTimerExpire => write!(f, "Timer_KeepaliveTimerExpire"),
            TimerEvent::DelayOpenTimerExpire => write!(f, "Timer_DelayOpenTimerExpire"),
            TimerEvent::IdleHoldTimerExpire => write!(f, "Timer_IdleHoldTimerExpire"),
        }
    }
}

impl Into<u8> for TimerEvent {
    fn into(self) -> u8 {
        match self {
            TimerEvent::ConnectRetryTimerExpire => 9,
            TimerEvent::HoldTimerExpire => 10,
            TimerEvent::KeepaliveTimerExpire => 11,
            TimerEvent::DelayOpenTimerExpire => 12,
            TimerEvent::IdleHoldTimerExpire => 13,
        }
    }
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

impl std::fmt::Display for TcpConnectionEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TcpConnectionEvent::TcpConnectionValid => write!(f, "Connection_TcpConnectionValid"),
            TcpConnectionEvent::TcpCRInvalid => write!(f, "Connection_TcpCRInvalid"),
            TcpConnectionEvent::TcpCRAcked(_) => write!(f, "Connection_TcpCRAcked"),
            TcpConnectionEvent::TcpConnectionConfirmed(_) => {
                write!(f, "Connection_TcpConnectionConfirmed")
            }
            TcpConnectionEvent::TcpConnectionFail => write!(f, "Connection_TcpConnectionFail"),
        }
    }
}

impl Into<u8> for TcpConnectionEvent {
    fn into(self) -> u8 {
        match self {
            TcpConnectionEvent::TcpConnectionValid => 14,
            TcpConnectionEvent::TcpCRInvalid => 15,
            TcpConnectionEvent::TcpCRAcked(_) => 16,
            TcpConnectionEvent::TcpConnectionConfirmed(_) => 17,
            TcpConnectionEvent::TcpConnectionFail => 18,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum BgpMessageEvent {
    BgpOpen(Message),
    #[allow(unused)]
    BgpOpenWithDelayOpenTimerRunning,
    BgpHeaderError(MessageHeaderError),
    BgpOpenMsgErr(OpenMessageError),
    #[allow(unused)]
    OpenCollisionDump,
    NotifMsgVerErr,
    NotifMsg(Message),
    KeepAliveMsg,
    UpdateMsg(Message),
    UpdateMsgErr(UpdateMessageError),
    RouteRefreshMsg(Message),
    RouteRefreshMsgErr,
}

impl std::fmt::Display for BgpMessageEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BgpMessageEvent::BgpOpen(_) => write!(f, "Message_BGPOpen"),
            BgpMessageEvent::BgpOpenWithDelayOpenTimerRunning => {
                write!(f, "Message_BgpOpenWithDelayOpenTimerRunning")
            }
            BgpMessageEvent::BgpHeaderError(_) => write!(f, "Message_BgpHeaderError"),
            BgpMessageEvent::BgpOpenMsgErr(_) => write!(f, "Message_BgpOpenMsgErr"),
            BgpMessageEvent::OpenCollisionDump => write!(f, "Message_OpenCollisionDump"),
            BgpMessageEvent::NotifMsgVerErr => write!(f, "Message_NotifMsgVerErr"),
            BgpMessageEvent::NotifMsg(_) => write!(f, "Message_NotifMsg"),
            BgpMessageEvent::KeepAliveMsg => write!(f, "Message_KeepAliveMsg"),
            BgpMessageEvent::UpdateMsg(_) => write!(f, "Message_UpdateMsg"),
            BgpMessageEvent::UpdateMsgErr(_) => write!(f, "Message_UpdateMsgErr"),
            BgpMessageEvent::RouteRefreshMsg(_) => write!(f, "Message_RouteRefreshMsg"),
            BgpMessageEvent::RouteRefreshMsgErr => write!(f, "Message_RouteRefreshMsgErr"),
        }
    }
}

impl Into<u8> for BgpMessageEvent {
    fn into(self) -> u8 {
        match self {
            BgpMessageEvent::BgpOpen(_) => 19,
            BgpMessageEvent::BgpOpenWithDelayOpenTimerRunning => 20,
            BgpMessageEvent::BgpHeaderError(_) => 21,
            BgpMessageEvent::BgpOpenMsgErr(_) => 22,
            BgpMessageEvent::OpenCollisionDump => 23,
            BgpMessageEvent::NotifMsgVerErr => 24,
            BgpMessageEvent::NotifMsg(_) => 25,
            BgpMessageEvent::KeepAliveMsg => 26,
            BgpMessageEvent::UpdateMsg(_) => 27,
            BgpMessageEvent::UpdateMsgErr(_) => 28,
            BgpMessageEvent::RouteRefreshMsg(_) => 100,
            BgpMessageEvent::RouteRefreshMsgErr => 101,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ControlEvent {
    AddPeer(NeighborConfig),
    Health,
}

impl std::fmt::Display for ControlEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlEvent::AddPeer(_) => write!(f, "Control_AddPeer"),
            ControlEvent::Health => write!(f, "Health"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RibEvent {
    AddPeer{neighbor: NeighborPair, rib_event_tx: Sender<RibEvent>},
    AddNetwork(Vec<String>),
    InstallPaths(Vec<Path>),
    DropPaths(Vec<Prefix>),
}
