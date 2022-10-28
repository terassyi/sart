use crate::bgp::capability;
use crate::bgp::error::{Error, MessageHeaderError};
use crate::bgp::family::{AddressFamily, Afi, Safi};
use crate::bgp::packet::attribute::Attribute;
use crate::bgp::packet::capability::Capability;
use crate::bgp::packet::prefix::Prefix;
use crate::bgp::server::Bgp;
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
    pub const VERSION: u8 = 4;
    pub const HEADER_LENGTH: u16 = 19;
    pub const MAX_LENGTH: u16 = 4096;
    pub const EXTENDED_MAX_LENGTH: u16 = 65535;
    pub const MARKER: u128 = 0xffff_ffff_ffff_ffff_ffff_ffff_ffff_ffff;
    pub const AS_TRANS: u32 = 23456;
    pub const OPTION_TYPE_CAPABILITIES: u8 = 2;
    pub const OPTION_TYPE_EXTENDED_LENGTH: u8 = 255;
}

impl<'a> Into<MessageType> for &'a Message {
    fn into(self) -> MessageType {
        match self {
            Message::Open { .. } => MessageType::Open,
            Message::Update { .. } => MessageType::Update,
            Message::Keepalive => MessageType::Keepalive,
            Message::Notification { .. } => MessageType::Notification,
            Message::RouteRefresh { .. } => MessageType::RouteRefresh,
        }
    }
}

impl Into<MessageType> for Message {
    fn into(self) -> MessageType {
        match self {
            Message::Open { .. } => MessageType::Open,
            Message::Update { .. } => MessageType::Update,
            Message::Keepalive => MessageType::Keepalive,
            Message::Notification { .. } => MessageType::Notification,
            Message::RouteRefresh { .. } => MessageType::RouteRefresh,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageType {
    Open = 1,
    Update = 2,
    Notification = 3,
    Keepalive = 4,
    RouteRefresh = 5,
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
    MessageHeader = 1,
    OpenMessage = 2,
    UpdateMessage = 3,
    HoldTimerExpired = 4,
    FiniteStateMachine = 5,
    Cease = 6,
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
            NotificationSubCode::ConnectionNotSynchronized => 1,
            NotificationSubCode::BadMessageLength => 2,
            NotificationSubCode::BadMessageType => 3,
            NotificationSubCode::UnsupportedVersionNumber => 1,
            NotificationSubCode::BadPeerAS => 2,
            NotificationSubCode::BadBGPIdentifier => 3,
            NotificationSubCode::UnsupportedOptionalParameter => 4,
            NotificationSubCode::UnacceptableHoldTime => 6,
            NotificationSubCode::MalformedAttributeList => 1,
            NotificationSubCode::UnrecognizedWellknownAttribute => 2,
            NotificationSubCode::MissingWellknownAttribute => 3,
            NotificationSubCode::AttributeFlagsError => 4,
            NotificationSubCode::AttributeLengthError => 5,
            NotificationSubCode::InvalidOriginAttribute => 6,
            NotificationSubCode::InvalidNextHopAttribute => 8,
            NotificationSubCode::OptionalAttributeError => 9,
            NotificationSubCode::InvalidNetworkField => 10,
            NotificationSubCode::MalformedASPath => 11,
        }
    }
}

pub(crate) struct MessageBuilder {
    msg_type: MessageType,
    // open
    version: u8,
    asn: u32,
    hold_time: u16,
    router_id: Ipv4Addr,
    capabilities: Vec<Capability>,
    // update
    withdrawn_routes: Vec<Prefix>,
    attributes: Vec<Attribute>,
    nlri: Vec<Prefix>,
    // notification
    code: NotificationCode,
    subcode: Option<NotificationSubCode>,
    data: Vec<u8>,
    // routerefresh
    family: AddressFamily,
}

impl MessageBuilder {
    pub fn builder(msg_type: MessageType) -> Self {
        Self {
            msg_type,
            version: Message::VERSION,
            asn: Message::AS_TRANS,
            hold_time: Bgp::DEFAULT_HOLD_TIME as u16,
            router_id: Ipv4Addr::new(0, 0, 0, 0),
            capabilities: Vec::new(),
            withdrawn_routes: Vec::new(),
            attributes: Vec::new(),
            nlri: Vec::new(),
            code: NotificationCode::FiniteStateMachine, // default
            subcode: None,
            data: Vec::new(),
            family: AddressFamily {
                afi: Afi::IPv4,
                safi: Safi::Unicast,
            },
        }
    }

    pub fn build(&self) -> Result<Message, Error> {
        match self.msg_type {
            MessageType::Open => Ok(Message::Open {
                version: self.version,
                as_num: self.asn,
                hold_time: self.hold_time,
                identifier: self.router_id,
                capabilities: self.capabilities.clone(),
            }),
            // MessageType::Update => {
            //     Ok(Message::Update {
            //         withdrawn_routes: self.withdrawn_routes.clone(),
            //         attributes: self.attributes.clone(),
            //         nlri: self.nlri.clone(),
            //     })
            // },
            MessageType::Keepalive => Ok(Message::Keepalive),
            // MessageType::Notification => {
            //     let code = match self.code {
            //         Some(code) => code,
            //         None => return Err(Error::MissingMessageField),
            //     };
            //     match &self.data {
            //         Some(data) => Ok(Message::Notification { code, subcode: self.subcode, data: data.clone() }),
            //         None => Ok(Message::Notification { code, subcode: self.subcode, data: Vec::new() }),
            //     }
            // },
            // MessageType::RouteRefresh => {
            //     let family = match self.family {
            //         Some(family) => family,
            //         None => return Err(Error::MissingMessageField),
            //     };
            //     Ok(Message::RouteRefresh { family })
            // },
            _ => Ok(Message::Keepalive),
        }
    }

    pub fn version(&mut self, version: u8) -> Result<&mut Self, Error> {
        if self.msg_type != MessageType::Open {
            return Err(Error::InvalidMessageField);
        }
        self.version = version;
        Ok(self)
    }

    pub fn asn(&mut self, asn: u32) -> Result<&mut Self, Error> {
        if self.msg_type != MessageType::Open {
            return Err(Error::InvalidMessageField);
        }
        self.asn = asn;
        Ok(self)
    }

    pub fn hold_time(&mut self, hold_time: u16) -> Result<&mut Self, Error> {
        if self.msg_type != MessageType::Open {
            return Err(Error::InvalidMessageField);
        }
        self.hold_time = hold_time;
        Ok(self)
    }

    pub fn identifier(&mut self, router_id: Ipv4Addr) -> Result<&mut Self, Error> {
        if self.msg_type != MessageType::Open {
            return Err(Error::InvalidMessageField);
        }
        self.router_id = router_id;
        Ok(self)
    }

    pub fn capability(&mut self, capability: &capability::Capability) -> Result<&mut Self, Error> {
        if self.msg_type != MessageType::Open {
            return Err(Error::InvalidMessageField);
        }
        self.capabilities.push(capability.into());
        Ok(self)
    }

    pub fn withdrawn_routes(&mut self, prefixes: Vec<Prefix>) -> Result<&mut Self, Error> {
        if self.msg_type != MessageType::Update {
            return Err(Error::InvalidMessageField);
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::Capability;
    use super::Message;
    use super::MessageBuilder;
    use super::MessageType;
    use super::*;
    use crate::bgp::capability;
    use crate::bgp::error::MessageHeaderError;
    use crate::bgp::family::{AddressFamily, Afi, Safi};
    use rstest::rstest;
    use std::net::Ipv4Addr;

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

    #[rstest(
        asn,
        hold_time,
        router_id,
        capabilities,
        expected,
        case(
            65100,
            180,
            Ipv4Addr::new(10, 10, 3, 1),
            vec![],
            Message::Open{version:4, as_num: 65100, hold_time:180, identifier:Ipv4Addr::new(10,10,3,1), capabilities: vec![] }
        ),
        case(
            65100,
            180,
            Ipv4Addr::new(10, 10, 3, 1),
            vec![
                capability::Capability::MultiProtocol(capability::MultiProtocol::new(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast })),
            ],
            Message::Open{version:4, as_num: 65100, hold_time:180, identifier:Ipv4Addr::new(10,10,3,1), capabilities: vec![
                Capability::MultiProtocol(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}),
            ] }
        ),
        // case(
        //     2621441,
        //     180,
        //     Ipv4Addr::new(1, 1, 1, 1),
        //     vec![
        //         capability::Capability::MultiProtocol(capability::MultiProtocol::new(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast })),
        //     ],
        //     Message::Open{version:4, as_num: 65100, hold_time:180, identifier:Ipv4Addr::new(10,10,3,1), capabilities: vec![
        //         Capability::MultiProtocol(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}),
        //     ] }
        // ),
    )]
    fn works_message_builder_open(
        asn: u32,
        hold_time: u16,
        router_id: Ipv4Addr,
        capabilities: Vec<capability::Capability>,
        expected: Message,
    ) {
        let mut builder = MessageBuilder::builder(MessageType::Open);
        let mut builder = builder
            .asn(asn)
            .unwrap()
            .hold_time(hold_time)
            .unwrap()
            .identifier(router_id)
            .unwrap();
        for cap in capabilities.iter() {
            builder.capability(cap).unwrap();
        }
        let msg = builder.build().unwrap();
        assert_eq!(expected, msg);
    }
}
