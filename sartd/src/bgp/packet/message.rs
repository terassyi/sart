use crate::bgp::error::{Error, MessageHeaderError};
use crate::bgp::family::AddressFamily;
use crate::bgp::packet::attribute::Attribute;
use crate::bgp::packet::capability::Capability;
use crate::bgp::packet::prefix::Prefix;
use std::convert::TryFrom;
use std::net::Ipv4Addr;

pub(crate) struct Builder {}

// https://www.rfc-editor.org/rfc/rfc4271#section-4.1
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Message {
    // https://www.rfc-editor.org/rfc/rfc4271#section-4.2
    Open {
        version: u8,
        // to allow 4 bytes AS number capability, as_num field is u32
        as_num: u32,
        hold_time: u16,
        identifier: Ipv4Addr, // router id: this is ipv4 address format
        capabilities: Vec<Capability>,
    },
    // https://www.rfc-editor.org/rfc/rfc4271#section-4.3
    Update {
        withdrawn_routes: Option<Vec<Prefix>>,
        attributes: Option<Vec<Attribute>>,
        nlri: Option<Vec<Prefix>>,
    },
    // https://www.rfc-editor.org/rfc/rfc4271#section-4.4
    Keepalive,
    // https://www.rfc-editor.org/rfc/rfc4271#section-4.5
    Notification {
        code: NotificationCode,
        subcode: Option<NotificationSubCode>,
        data: Vec<u8>,
    },
    // https://datatracker.ietf.org/doc/html/rfc2918#section-3
    RouteRefresh {
        family: AddressFamily,
    },
}

impl Message {
    pub const HEADER_LENGTH: u16 = 19;
    pub const MAX_LENGTH: u16 = 4096;
    pub const EXTENDED_MAX_LENGTH: u16 = 65535;
    pub const MARKER: u128 = 0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff;
    pub const AS_TRANS: u32 = 23456;
    pub const OPTION_TYPE_CAPABILITIES: u8 = 2;
    pub const OPTION_TYPE_EXTENDED_LENGTH: u8 = 255;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageType {
    Open,
    Update,
    Notification,
    Keepalive,
    RouteRefresh,
}

impl TryFrom<u8> for MessageType {
    type Error = MessageHeaderError;
    fn try_from(from: u8) -> Result<Self, Self::Error> {
        match from {
            1 => Ok(Self::Open),
            2 => Ok(Self::Update),
            3 => Ok(Self::Notification),
            4 => Ok(Self::Keepalive),
            5 => Ok(Self::RouteRefresh),
            _ => Err(MessageHeaderError::BadMessageType { val: from }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationCode {
    MessageHeader,
    OpenMessage,
    UpdateMessage,
    HoldTimerExpired,
    FiniteStateMachine,
    Cease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationSubCode {
    // for MessageHeader
    ConnectionNotSynchronized,
    BadMessageLength,
    BadMessageType,
    // for OpenMessage
    UnsupportedVersionNumber,
    BadPeerAS,
    BadBGPIdentifier,
    UnsupportedOptionalParameter,
    UnacceptableHoldTime,
    // for UpdateMessage
    MalformedAttributeList,
    UnrecognizedWellknownAttribute,
    MissingWellknownAttribute,
    AttributeFlagsError,
    AttributeLengthError,
    InvalidOriginAttribute,
    InvalidNextHopAttribute,
    OptionalAttributeError,
    InvalidNetworkField,
    MalformedASPath,
}

impl TryFrom<u8> for NotificationCode {
    type Error = Error;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::MessageHeader),
            2 => Ok(Self::OpenMessage),
            3 => Ok(Self::UpdateMessage),
            4 => Ok(Self::HoldTimerExpired),
            5 => Ok(Self::FiniteStateMachine),
            6 => Ok(Self::Cease),
            _ => Err(Error::UnrecognizedNotificationCode),
        }
    }
}

impl Into<u8> for NotificationCode {
    fn into(self) -> u8 {
        match self {
            Self::MessageHeader => 1,
            Self::OpenMessage => 2,
            Self::UpdateMessage => 3,
            Self::HoldTimerExpired => 4,
            Self::FiniteStateMachine => 5,
            Self::Cease => 6,
        }
    }
}

impl NotificationSubCode {
    pub fn try_from_with_code(value: u8, code: NotificationCode) -> Result<Option<Self>, Error> {
        match code {
            NotificationCode::MessageHeader => match value {
                1 => Ok(Some(Self::ConnectionNotSynchronized)),
                2 => Ok(Some(Self::BadMessageLength)),
                3 => Ok(Some(Self::BadMessageType)),
                _ => Err(Error::UnrecognizedNotificationSubCode),
            },
            NotificationCode::OpenMessage => match value {
                1 => Ok(Some(Self::UnsupportedVersionNumber)),
                2 => Ok(Some(Self::BadPeerAS)),
                3 => Ok(Some(Self::BadBGPIdentifier)),
                4 => Ok(Some(Self::UnsupportedOptionalParameter)),
                6 => Ok(Some(Self::UnacceptableHoldTime)),
                _ => Err(Error::UnrecognizedNotificationSubCode),
            },
            NotificationCode::UpdateMessage => match value {
                1 => Ok(Some(Self::MalformedAttributeList)),
                2 => Ok(Some(Self::UnrecognizedWellknownAttribute)),
                3 => Ok(Some(Self::MissingWellknownAttribute)),
                4 => Ok(Some(Self::AttributeFlagsError)),
                5 => Ok(Some(Self::AttributeLengthError)),
                6 => Ok(Some(Self::InvalidOriginAttribute)),
                8 => Ok(Some(Self::InvalidNextHopAttribute)),
                9 => Ok(Some(Self::OptionalAttributeError)),
                10 => Ok(Some(Self::InvalidNetworkField)),
                11 => Ok(Some(Self::MalformedASPath)),
                _ => Err(Error::UnrecognizedNotificationSubCode),
            },
            _ => Ok(None),
        }
    }
}

impl Into<u8> for NotificationSubCode {
    fn into(self) -> u8 {
        match self {
            Self::ConnectionNotSynchronized => 1,
            Self::BadMessageLength => 2,
            Self::BadMessageType => 3,
            Self::UnsupportedVersionNumber => 1,
            Self::BadPeerAS => 2,
            Self::BadBGPIdentifier => 3,
            Self::UnsupportedOptionalParameter => 4,
            Self::UnacceptableHoldTime => 6,
            Self::MalformedAttributeList => 1,
            Self::UnrecognizedWellknownAttribute => 2,
            Self::MissingWellknownAttribute => 3,
            Self::AttributeFlagsError => 4,
            Self::AttributeLengthError => 5,
            Self::InvalidOriginAttribute => 6,
            Self::InvalidNextHopAttribute => 8,
            Self::OptionalAttributeError => 9,
            Self::InvalidNetworkField => 10,
            Self::MalformedASPath => 11,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MessageType;
    use crate::bgp::error::MessageHeaderError;
    use rstest::rstest;

    #[rstest(
        input,
        expected,
        case(1, MessageType::Open),
        case(2, MessageType::Update),
        case(3, MessageType::Notification),
        case(4, MessageType::Keepalive),
        case(5, MessageType::RouteRefresh)
    )]
    fn work_message_type_try_from_test(input: u8, expected: MessageType) {
        let res = MessageType::try_from(input).unwrap();
        assert_eq!(expected, res);
    }

    #[rstest(input, expected,
		case(0, MessageHeaderError::BadMessageType { val: 0 }),
		case(6, MessageHeaderError::BadMessageType { val: 6 }),
	)]
    fn failed_message_type_try_from_test(input: u8, expected: MessageHeaderError) {
        match MessageType::try_from(input) {
            Ok(_) => assert!(false),
            Err(e) => assert_eq!(expected, e),
        }
    }
}
