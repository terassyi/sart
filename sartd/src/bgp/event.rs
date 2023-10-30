use std::net::IpAddr;
use std::net::Ipv4Addr;

use ipnet::IpNet;
use tokio::net::TcpStream;
use tokio::sync::mpsc::Sender;

use super::config::NeighborConfig;
use super::error::MessageHeaderError;
use super::error::OpenMessageError;
use super::error::UpdateMessageError;
use super::family::AddressFamily;
use super::packet::attribute::Attribute;
use super::packet::message::Message;
use super::path::Path;
use super::peer::neighbor::NeighborPair;
use super::rib::RibKind;

#[derive(Debug)]
pub(crate) enum Event {
    Admin(AdministrativeEvent),
    Timer(TimerEvent),
    Connection(TcpConnectionEvent),
    Message(BgpMessageEvent),
    Control(ControlEvent),
    Api(PeerLevelApiEvent),
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

impl From<&Event> for u8 {
    fn from(val: &Event) -> Self {
        match *val {
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
			Event::Connection(TcpConnectionEvent::TcpCRAcked) => 16,
			Event::Connection(TcpConnectionEvent::TcpConnectionConfirmed(_)) => 17,
			Event::Connection(TcpConnectionEvent::TcpConnectionFail) => 18,
			Event::Message(BgpMessageEvent::BgpOpen { local_port: _, peer_port: _, msg: _ }) => 19,
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
            Event::Api(e) => write!(f, "{}", e),
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
			AdministrativeEvent::ManualStart => write!(f, "Admin::ManualStart"),
			AdministrativeEvent::ManualStop => write!(f, "Admin::ManualStop"),
			AdministrativeEvent::AutomaticStart => write!(f, "Admin::AutomaticStart"),
			AdministrativeEvent::ManualStartWithPassiveTcpEstablishment => write!(f, "Admin::ManualStartWithPassiveTcpEstablishment"),
			AdministrativeEvent::AutomaticStartWithPassiveTcpEstablishment => write!(f, "Admin::AutomaticStartWithPassiveTcpEstablishment"),
			AdministrativeEvent::AutomaticStartWithDampPeerOscillations => write!(f, "Admin::AutomaticStartWithDampPeerOscillations"),
			AdministrativeEvent::AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment => write!(f, "Admin::AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment"),
			AdministrativeEvent::AutomaticStop => write!(f, "Admin::AutomaticStop"),
        }
    }
}

impl From<AdministrativeEvent> for u8 {
    fn from(val: AdministrativeEvent) -> Self {
        match val {
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
            TimerEvent::ConnectRetryTimerExpire => write!(f, "Timer::ConnectRetryTimerExpire"),
            TimerEvent::HoldTimerExpire => write!(f, "Timer::HoldTimerExpire"),
            TimerEvent::KeepaliveTimerExpire => write!(f, "Timer::KeepaliveTimerExpire"),
            TimerEvent::DelayOpenTimerExpire => write!(f, "Timer::DelayOpenTimerExpire"),
            TimerEvent::IdleHoldTimerExpire => write!(f, "Timer::IdleHoldTimerExpire"),
        }
    }
}

impl From<TimerEvent> for u8 {
    fn from(val: TimerEvent) -> Self {
        match val {
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
    TcpCRAcked,
    TcpConnectionConfirmed(TcpStream),
    TcpConnectionFail,
}

impl std::fmt::Display for TcpConnectionEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TcpConnectionEvent::TcpConnectionValid => write!(f, "Connection::TcpConnectionValid"),
            TcpConnectionEvent::TcpCRInvalid => write!(f, "Connection::TcpCRInvalid"),
            TcpConnectionEvent::TcpCRAcked => write!(f, "Connection::TcpCRAcked"),
            TcpConnectionEvent::TcpConnectionConfirmed(_) => {
                write!(f, "Connection::TcpConnectionConfirmed")
            }
            TcpConnectionEvent::TcpConnectionFail => write!(f, "Connection_TcpConnectionFail"),
        }
    }
}

impl From<TcpConnectionEvent> for u8 {
    fn from(val: TcpConnectionEvent) -> Self {
        match val {
            TcpConnectionEvent::TcpConnectionValid => 14,
            TcpConnectionEvent::TcpCRInvalid => 15,
            TcpConnectionEvent::TcpCRAcked => 16,
            TcpConnectionEvent::TcpConnectionConfirmed(_) => 17,
            TcpConnectionEvent::TcpConnectionFail => 18,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum BgpMessageEvent {
    BgpOpen {
        local_port: u16,
        peer_port: u16,
        msg: Message,
    },
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
            BgpMessageEvent::BgpOpen {
                local_port: _,
                peer_port: _,
                msg: _,
            } => write!(f, "Message::BGPOpen"),
            BgpMessageEvent::BgpOpenWithDelayOpenTimerRunning => {
                write!(f, "Message::BgpOpenWithDelayOpenTimerRunning")
            }
            BgpMessageEvent::BgpHeaderError(_) => write!(f, "Message::BgpHeaderError"),
            BgpMessageEvent::BgpOpenMsgErr(_) => write!(f, "Message::BgpOpenMsgErr"),
            BgpMessageEvent::OpenCollisionDump => write!(f, "Message::OpenCollisionDump"),
            BgpMessageEvent::NotifMsgVerErr => write!(f, "Message::NotifMsgVerErr"),
            BgpMessageEvent::NotifMsg(_) => write!(f, "Message::NotifMsg"),
            BgpMessageEvent::KeepAliveMsg => write!(f, "Message::KeepAliveMsg"),
            BgpMessageEvent::UpdateMsg(_) => write!(f, "Message::UpdateMsg"),
            BgpMessageEvent::UpdateMsgErr(_) => write!(f, "Message::UpdateMsgErr"),
            BgpMessageEvent::RouteRefreshMsg(_) => write!(f, "Message::RouteRefreshMsg"),
            BgpMessageEvent::RouteRefreshMsgErr => write!(f, "Message::RouteRefreshMsgErr"),
        }
    }
}

impl From<BgpMessageEvent> for u8 {
    fn from(val: BgpMessageEvent) -> Self {
        match val {
            BgpMessageEvent::BgpOpen {
                local_port: _,
                peer_port: _,
                msg: _,
            } => 19,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ControlEvent {
    GetBgpInfo,
    GetPeer(IpAddr),
    ListPeer,
    GetPath(AddressFamily),
    GetNeighborPath(RibKind, IpAddr, AddressFamily),
    GetPathByPrefix(IpNet, AddressFamily),
    SetAsn(u32),
    SetRouterId(Ipv4Addr),
    AddPeer(NeighborConfig),
    DeletePeer(IpAddr),
    AddPath(Vec<IpNet>, Vec<Attribute>),
    DeletePath(AddressFamily, Vec<IpNet>),
    Health,
}

impl std::fmt::Display for ControlEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControlEvent::GetBgpInfo => write!(f, "Control::GetBgpInfo"),
            ControlEvent::GetPeer(_) => write!(f, "Control::GetPeer"),
            ControlEvent::ListPeer => write!(f, "Control::ListPeer"),
            ControlEvent::GetPath(_) => write!(f, "Control::GetPath"),
            ControlEvent::GetNeighborPath(_, _, _) => write!(f, "Control::GetNeighborPath"),
            ControlEvent::GetPathByPrefix(_, _) => write!(f, "Control::GetPathByPrefix"),
            ControlEvent::SetAsn(_) => write!(f, "Control::SetAsn"),
            ControlEvent::SetRouterId(_) => write!(f, "Control::SetRouterId"),
            ControlEvent::AddPeer(_) => write!(f, "Control::AddPeer"),
            ControlEvent::DeletePeer(_) => write!(f, "Control::DeletePeer"),
            ControlEvent::AddPath(_, _) => write!(f, "Control::AddPath"),
            ControlEvent::DeletePath(_, _) => write!(f, "Control::DeletePath"),
            ControlEvent::Health => write!(f, "Control::Health"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum RibEvent {
    SetAsn(u32),
    SetRouterId(Ipv4Addr),
    AddPeer(NeighborPair, Sender<RibEvent>),
    DeletePeer(NeighborPair),
    Init(AddressFamily, NeighborPair),
    Flush(AddressFamily, NeighborPair),
    AddPath(Vec<IpNet>, Vec<Attribute>),   // from local
    DeletePath(AddressFamily, Vec<IpNet>), // from local
    InstallPaths(NeighborPair, Vec<Path>), // from peer
    DropPaths(NeighborPair, AddressFamily, Vec<(IpNet, u64)>), // from peer
    Advertise(Vec<Path>),
    Withdraw(Vec<IpNet>),
    GetPath(AddressFamily),
    GetPathByPrefix(IpNet, AddressFamily),
}

impl std::fmt::Display for RibEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RibEvent::SetAsn(_) => write!(f, "Rib::SetAsn"),
            RibEvent::SetRouterId(_) => write!(f, "Rib::SetRouterId"),
            RibEvent::AddPeer(_, _) => write!(f, "Rib::AddPeer"),
            RibEvent::DeletePeer(_) => write!(f, "Rib::DeletePeer"),
            RibEvent::Init(_, _) => write!(f, "Rib::Init"),
            RibEvent::Flush(_, _) => write!(f, "Rib::Flush"),
            RibEvent::AddPath(_, _) => write!(f, "Rib::AddPath"),
            RibEvent::DeletePath(_, _) => write!(f, "Rib::DeletePath"),
            RibEvent::InstallPaths(_, _) => write!(f, "Rib::InstallPaths"),
            RibEvent::DropPaths(_, _, _) => write!(f, "Rib::DropPaths"),
            RibEvent::Advertise(_) => write!(f, "Rib::Advertise"),
            RibEvent::Withdraw(_) => write!(f, "Rib::Withdraw"),
            RibEvent::GetPath(_) => write!(f, "Rib::GetPath"),
            RibEvent::GetPathByPrefix(_, _) => write!(f, "Rib::GetPathByPrefix"),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum PeerLevelApiEvent {
    GetPeer,
    GetPath(RibKind, AddressFamily),
}

impl std::fmt::Display for PeerLevelApiEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PeerLevelApiEvent::GetPeer => write!(f, "PeerLevelApi::GetPeer"),
            PeerLevelApiEvent::GetPath(_, _) => write!(f, "PeerLevelApi::GetPath"),
        }
    }
}
