use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("")]
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
    InvalidEvent{val: u8},
}

// https://www.rfc-editor.org/rfc/rfc1771#section-6.1
#[derive(Debug, Error, PartialEq)]
pub(crate) enum MessageHeaderError {
    #[error("Connection not synchronized")]
    ConnectionNotSynchronized,
    #[error("Bad message length {length:?}")]
    BadMessageLength { length: u16 },
    #[error("Bad message type {val:?}")]
    BadMessageType { val: u8 },
}

// https://www.rfc-editor.org/rfc/rfc4271#section-6.2
#[derive(Debug, Error, PartialEq)]
pub(crate) enum OpenMessageError {
    #[error("Unsupported version number")]
    UnsupportedVersionNumber,
    #[error("Invalid peer AS")]
    InvalidPeerAS,
    #[error("Unacceptable hold time")]
    UnacceptableHoldTime,
    #[error("Unsupported optional parameters")]
    UnsupportedOptionalParameter,
    #[error("Unspecific capability")]
    Unspecific,
}

// https://www.rfc-editor.org/rfc/rfc4271#section-6.3
#[derive(Debug, Error, PartialEq)]
pub(crate) enum UpdateMessageError {
    #[error("Malformed attribute list")]
    MalformedAttributeList,
    #[error("Optional attribute error")]
    OptionalAttributeError,
    #[error("Attribute flags error")]
    AttributeFlagsError,
    #[error("Missing well known attribute")]
    MissingWellKnownAttribute,
    #[error("Invalid ORIGIN attribute")]
    InvalidOriginAttribute,
    #[error("Invalid NEXT_HOP attribute")]
    InvalidNextHopAttribute,
    #[error("Malformed AS path")]
    MalformedASPath,
    #[error("Invalid network field")]
    InvalidNetworkField,
}

#[derive(Debug, Error)]
pub(crate) enum ConfigError {
    #[error("failed to load")]
    FailedToLoad,
}
