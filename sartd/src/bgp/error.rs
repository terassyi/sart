use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("system error")]
    System,
    #[error("message header error")]
    MessageHeader(#[from] MessageHeaderError),
    #[error("OPEN message error")]
    OpenMessage(#[from] OpenMessageError),
    #[error("UPDATE message error")]
    UpdateMessage(#[from] UpdateMessageError),
    #[error("ROUTE-REFRESH message error")]
    RouteRefreshMessageError,
    #[error("Unrecognized Notification code")]
    UnrecognizedNotificationCode,
    #[error("Unrecognized Notification subcode")]
    UnrecognizedNotificationSubCode,
    #[error("Hold timer expired")]
    HoldTimerExpired,
    #[error("Finite state machine error")]
    FiniteStateMachine,
    #[error("Cease")]
    Cease,
    #[error("std::io::Error")]
    StdIoErr(#[from] std::io::Error),
    #[error("config error")]
    Config(#[from] ConfigError),
    #[error("Invalid event {val:?}")]
    InvalidEvent { val: u8 },
    #[error("control error")]
    Control(#[from] ControlError),
    #[error("missing message fields")]
    MissingMessageField,
    #[error("invalid message field")]
    InvalidMessageField,
    #[error("undesired message")]
    UndesiredMessage,
    #[error("peer error")]
    Peer(#[from] PeerError),
    #[error("rib error")]
    Rib(#[from] RibError),
}

// https://www.rfc-editor.org/rfc/rfc1771#section-6.1
#[derive(Debug, Error, PartialEq, Clone)]
pub(crate) enum MessageHeaderError {
    #[error("Connection not synchronized")]
    ConnectionNotSynchronized,
    #[error("Bad message length {length:?}")]
    BadMessageLength { length: u16 },
    #[error("Bad message type {val:?}")]
    BadMessageType { val: u8 },
}

impl Into<u8> for MessageHeaderError {
    fn into(self) -> u8 {
        match self {
            Self::ConnectionNotSynchronized => 1,
            Self::BadMessageLength { length: _ } => 2,
            Self::BadMessageType { val: _ } => 3,
        }
    }
}

// https://www.rfc-editor.org/rfc/rfc4271#section-6.2
#[derive(Debug, Error, PartialEq, Clone)]
pub(crate) enum OpenMessageError {
    #[error("Unsupported version number")]
    UnsupportedVersionNumber,
    #[error("Invalid peer AS")]
    BadPeerAS,
    #[error("Bad BGP identifier")]
    BadBGPIdentifier,
    #[error("Unsupported optional parameters")]
    UnsupportedOptionalParameter,
    #[error("Unacceptable hold time")]
    UnacceptableHoldTime,
    #[error("Unspecific capability")]
    Unspecific,
}

impl Into<u8> for OpenMessageError {
    fn into(self) -> u8 {
        match self {
            Self::UnsupportedVersionNumber => 1,
            Self::BadPeerAS => 2,
            Self::BadBGPIdentifier => 3,
            Self::UnsupportedOptionalParameter => 4,
            Self::UnacceptableHoldTime => 6,
            Self::Unspecific => 0,
        }
    }
}

// https://www.rfc-editor.org/rfc/rfc4271#section-6.3
#[derive(Debug, Error, PartialEq, Clone)]
pub(crate) enum UpdateMessageError {
    #[error("Malformed attribute list")]
    MalformedAttributeList,
    #[error("Unrecognized well known attribute")]
    UnrecognizedWellknownAttribute(u8),
    #[error("Missing well known attribute")]
    MissingWellKnownAttribute(u8),
    #[error("Attribute flags error")]
    AttributeFlagsError { code: u8, length: usize, value: u8 },
    #[error("Attribute length error")]
    AttributeLengthError { code: u8, length: usize, value: u8 },
    #[error("Invalid ORIGIN attribute")]
    InvalidOriginAttribute(u8),
    #[error("Invalid NEXT_HOP attribute")]
    InvalidNextHopAttribute,
    #[error("Optional attribute error")]
    OptionalAttributeError,
    #[error("Invalid network field")]
    InvalidNetworkField,
    #[error("Malformed AS path")]
    MalformedASPath,
}

impl Into<u8> for UpdateMessageError {
    fn into(self) -> u8 {
        match self {
            Self::MalformedAttributeList => 1,
            Self::UnrecognizedWellknownAttribute(_) => 2,
            Self::MissingWellKnownAttribute(_) => 3,
            Self::AttributeFlagsError {
                code: _,
                length: _,
                value: _,
            } => 4,
            Self::AttributeLengthError {
                code: _,
                length: _,
                value: _,
            } => 5,
            Self::InvalidOriginAttribute(u8) => 6,
            Self::InvalidNextHopAttribute => 8,
            Self::OptionalAttributeError => 9,
            Self::InvalidNetworkField => 10,
            Self::MalformedASPath => 11,
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum ConfigError {
    #[error("already configured")]
    AlreadyConfigured,
    #[error("failed to load")]
    FailedToLoad,
    #[error("invalid argument")]
    InvalidArgument,
    #[error("invalid data")]
    InvalidData,
}

#[derive(Debug, Error)]
pub(crate) enum ControlError {
    #[error("peer already exists")]
    PeerAlreadyExists,
    #[error("invalid data")]
    InvalidData,
    #[error("failed to send/recv channel")]
    FailedToSendRecvChannel,
}

#[derive(Debug, Error)]
pub(crate) enum PeerError {
    #[error("connection is not established")]
    ConnectionNotEstablished,
    #[error("failed to send message")]
    FailedToSendMessage,
    #[error("peer is down")]
    Down,
    #[error("duplicate connection")]
    DuplicateConnection,
}

#[derive(Debug, Error)]
pub(crate) enum RibError {
    #[error("invalid address family")]
    InvalidAddressFamily,
    #[error("address family is not set")]
    AddressFamilyNotSet,
    #[error("peer is already registered")]
    PeerAlreadyRegistered,
    #[error("peer not found")]
    PeerNotFound,
    #[error("loc-rib manager down")]
    ManagerDown,
    #[error("protocol is already registered in Loc-RIB")]
    ProtocolIsAlreadyRegistered,
    #[error("path not found")]
    PathNotFound,
    #[error("unhandlable event")]
    UnhandlableEvent,
}
