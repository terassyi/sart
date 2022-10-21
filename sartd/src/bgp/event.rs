use crate::bgp::error::Error;
use std::convert::TryFrom;
use std::convert::TryInto;

use super::config::NeighborConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Event {
    Admin(AdministrativeEvent),
    Timer(TimerEvent),
    Connection(TcpConnectionEvent),
    Message(BgpMessageEvent),
}

impl TryFrom<u8> for Event {
    type Error = Error;
    fn try_from(val: u8) -> Result<Self, Self::Error> {
        match val {
			1 => Ok(Self::Admin(AdministrativeEvent::ManualStart)),
			2 => Ok(Self::Admin(AdministrativeEvent::ManualStop)),
			3 => Ok(Self::Admin(AdministrativeEvent::AutomaticStart)),
			4 => Ok(Self::Admin(AdministrativeEvent::ManualStartWithPassiveTcpEstablishment)),
			5 => Ok(Self::Admin(AdministrativeEvent::AutomaticStartWithPassiveTcpEstablishment)),
			6 => Ok(Self::Admin(AdministrativeEvent::AutomaticStartWithDampPeerOscillations)),
			7 => Ok(Self::Admin(AdministrativeEvent::AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment)),
			8 => Ok(Self::Admin(AdministrativeEvent::AutomaticStop)),
			9 => Ok(Self::Timer(TimerEvent::ConnectRetryTimerExpire)),
			10 => Ok(Self::Timer(TimerEvent::HoldTimerExpire)),
			11 => Ok(Self::Timer(TimerEvent::KeepaliveTimerExpire)),
			12 => Ok(Self::Timer(TimerEvent::DelayOpenTimerExpire)),
			13 => Ok(Self::Timer(TimerEvent::IdleHoldTimerExpire)),
			14 => Ok(Self::Connection(TcpConnectionEvent::TcpConnectionValid)),
			15 => Ok(Self::Connection(TcpConnectionEvent::TcpCRInvalid)),
			16 => Ok(Self::Connection(TcpConnectionEvent::TcpCRAcked)),
			17 => Ok(Self::Connection(TcpConnectionEvent::TcpConnectionConfirmed)),
			18 => Ok(Self::Connection(TcpConnectionEvent::TcpConnectionFail)),
			19 => Ok(Self::Message(BgpMessageEvent::BgpOpen)),
			20 => Ok(Self::Message(BgpMessageEvent::BgpOpenWithDelayOpenTimerRunning)),
			21 => Ok(Self::Message(BgpMessageEvent::BgpHeaderError)),
			22 => Ok(Self::Message(BgpMessageEvent::BgpOpenMsgErr)),
			23 => Ok(Self::Message(BgpMessageEvent::OpenCollisionDump)),
			24 => Ok(Self::Message(BgpMessageEvent::NotifMsgVerErr)),
			25 => Ok(Self::Message(BgpMessageEvent::NotifMsg)),
			26 => Ok(Self::Message(BgpMessageEvent::KeepAliveMsg)),
			27 => Ok(Self::Message(BgpMessageEvent::UpdateMsg)),
			28 => Ok(Self::Message(BgpMessageEvent::UpdateMsgErr)),
			_ => Err(Error::InvalidEvent { val })
		}
    }
}

impl Into<u8> for Event {
    fn into(self) -> u8 {
        match self {
			Self::Admin(AdministrativeEvent::ManualStart) => 1,
			Self::Admin(AdministrativeEvent::ManualStop) => 2,
			Self::Admin(AdministrativeEvent::AutomaticStart) => 3,
			Self::Admin(AdministrativeEvent::ManualStartWithPassiveTcpEstablishment) => 4,
			Self::Admin(AdministrativeEvent::AutomaticStartWithPassiveTcpEstablishment) => 5,
			Self::Admin(AdministrativeEvent::AutomaticStartWithDampPeerOscillations) => 6,
			Self::Admin(AdministrativeEvent::AutomaticStartWithDampPeerOscillationsAndPassiveTcpEstablishment) => 7,
			Self::Admin(AdministrativeEvent::AutomaticStop) => 8,
			Self::Timer(TimerEvent::ConnectRetryTimerExpire) => 9,
			Self::Timer(TimerEvent::HoldTimerExpire) => 10,
			Self::Timer(TimerEvent::KeepaliveTimerExpire) => 11,
			Self::Timer(TimerEvent::DelayOpenTimerExpire) => 12,
			Self::Timer(TimerEvent::IdleHoldTimerExpire) => 13,
			Self::Connection(TcpConnectionEvent::TcpConnectionValid) => 14,
			Self::Connection(TcpConnectionEvent::TcpCRInvalid) => 15,
			Self::Connection(TcpConnectionEvent::TcpCRAcked) => 16,
			Self::Connection(TcpConnectionEvent::TcpConnectionConfirmed) => 17,
			Self::Connection(TcpConnectionEvent::TcpConnectionFail) => 18,
			Self::Message(BgpMessageEvent::BgpOpen) => 19,
			Self::Message(BgpMessageEvent::BgpOpenWithDelayOpenTimerRunning) => 20,
			Self::Message(BgpMessageEvent::BgpHeaderError) => 21,
			Self::Message(BgpMessageEvent::BgpOpenMsgErr) => 22,
			Self::Message(BgpMessageEvent::OpenCollisionDump) => 23,
			Self::Message(BgpMessageEvent::NotifMsgVerErr) => 24,
			Self::Message(BgpMessageEvent::NotifMsg) => 25,
			Self::Message(BgpMessageEvent::KeepAliveMsg) => 26,
			Self::Message(BgpMessageEvent::UpdateMsg) => 27,
			Self::Message(BgpMessageEvent::UpdateMsgErr) => 28,
			Self::Admin(RegisterPeer) => 100,
		}
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum TcpConnectionEvent {
    #[allow(unused)]
    TcpConnectionValid,
    #[allow(unused)]
    TcpCRInvalid,
    TcpCRAcked,
    TcpConnectionConfirmed,
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

#[derive(Debug)]
pub(crate) enum ControlEvent {
    RegisterPeer(NeighborConfig),
}
