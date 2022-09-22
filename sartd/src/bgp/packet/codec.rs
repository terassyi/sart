use std::net::Ipv4Addr;

use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::bgp::error::*;
use crate::bgp::family::AddressFamily;
use crate::bgp::packet::message::{Message, MessageType, NotificationCode, NotificationSubCode};
use crate::bgp::packet::prefix::Prefix;

use super::capability::Capability;

#[derive(Debug)]
pub(crate) struct Codec {
    family: AddressFamily,
}

impl Codec {
    pub fn new(family: AddressFamily) -> Self {
        Self { family }
    }
}

impl Decoder for Codec {
    type Item = Message;
    type Error = Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < Message::HEADER_LENGTH as usize {
            return Err(Error::MessageHeader(MessageHeaderError::BadMessageLength {
                length: src.len() as u16,
            }));
        }
        let marker = src.get_u128();
        let header_length = src.get_u16();
        if marker != Message::MARKER || header_length < Message::HEADER_LENGTH {
            return Err(Error::MessageHeader(MessageHeaderError::BadMessageLength {
                length: src.len() as u16,
            }));
        }
        let message_type = MessageType::try_from(src.get_u8())?;
        let message: Option<Self::Item> = match message_type {
            MessageType::Open => {
                let version = src.get_u8();
                let my_asn = src.get_u16();
                let hold_time = src.get_u16();
                let router_id = Ipv4Addr::from(src.get_u32());
                let mut capabilities = vec![];
                let capability_length = src.get_u16();
                if capability_length != src.remaining() as u16 {
                    return Err(Error::MessageHeader(MessageHeaderError::BadMessageLength { length: src.len() as u16}));
                }
                loop {
                    if src.remaining() == 0 {
                        break;
                    }
                    let cap = Capability::decode(src.get_u8(), src.get_u8(), src)?;
                    capabilities.push(cap)
                }

                Some(Message::Open {
                    version,
                    as_num: my_asn as u32,
                    hold_time,
                    identifier: router_id,
                    capabilities,
                })
            }
            MessageType::Update => {
                let withdrawn_routes_length = src.get_u16();
                let withdrawn_routes = if withdrawn_routes_length > 0 {
                    Some(vec![])
                } else {
                    None
                };
                let total_path_attribute_length = src.get_u16();
                let attributes = if total_path_attribute_length > 0 {
                    Some(vec![])
                } else {
                    None
                };
                let nlri = if src.remaining() > 0 {
                    Some(vec![])
                } else {
                    None
                };
                Some(Message::Update {
                    withdrawn_routes,
                    attributes,
                    nlri,
                })
            }
            MessageType::Keepalive => Some(Message::Keepalive {}),
            MessageType::Notification => {
                let code = NotificationCode::try_from(src.get_u8())?;
                let subcode = NotificationSubCode::try_from_with_code(src.get_u8(), code)?;
                Some(Message::Notification {
                    code,
                    subcode,
                    data: src.to_vec(),
                })
            }
            MessageType::RouteRefresh => {
                let family = AddressFamily::try_from(src.get_u32())
                    .map_err(|_| Error::RouteRefreshMessageError)?;
                Some(Message::RouteRefresh { family })
            }
        };
        Ok(message)
    }
}
