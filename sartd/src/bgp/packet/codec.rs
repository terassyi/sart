use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{Arc, RwLock};

use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::bgp::{error::*, capability};
use crate::bgp::family::{AddressFamily, Afi, Safi};
use crate::bgp::packet::message::{Message, MessageType, NotificationCode, NotificationSubCode};
use crate::bgp::packet::prefix::Prefix;
use crate::bgp::packet::attribute::Attribute;

use super::capability::Capability;

#[derive(Debug)]
pub(crate) struct Codec {
    family: AddressFamily,
    capabilities: Arc<RwLock<capability::CapabilitySet>>,
}

impl Codec {
    pub fn default() -> Self {
        Self {
            family: AddressFamily {
                afi: Afi::IPv4, 
                safi: Safi::Unicast,
            },
            capabilities: Arc::new(RwLock::new(HashMap::new())) }
    }
}

impl Decoder for Codec {
    type Item = Message;
    type Error = Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.remaining() < Message::HEADER_LENGTH as usize {
            return Err(Error::MessageHeader(MessageHeaderError::BadMessageLength {
                length: src.remaining() as u16,
            }));
        }
        let marker = src.get_u128();
        let header_length = src.get_u16();
        if marker != Message::MARKER || header_length < Message::HEADER_LENGTH {
            return Err(Error::MessageHeader(MessageHeaderError::BadMessageLength {
                length: header_length,
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
                let withdrawn_routes_length = src.get_u16() as usize;
                let withdrawn_routes = if withdrawn_routes_length > 0 {
                    let mut routes = Vec::new();
                    let remain = src.remaining();
                    while src.remaining() > remain - withdrawn_routes_length {
                        routes.push(Prefix::decode(self.family, false, src)?);
                    }
                    Some(routes)
                } else {
                    None
                };
                let total_path_attribute_length = src.get_u16() as usize;
                let attributes = if total_path_attribute_length > 0 {
                    let mut attributes = Vec::new();
                    let remain = src.remaining();
                    while src.remaining() > remain - (total_path_attribute_length) {
                        attributes.push(Attribute::decode(src)?);
                    }
                    Some(attributes)
                } else {
                    None
                };
                let nlri = if src.remaining() > 0 {
                    let mut prefixes = Vec::new();
                    while src.remaining() > 0 {
                        prefixes.push(Prefix::decode(self.family, false, src)?)
                    }
                    Some(prefixes)
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

impl Encoder<&Message> for Codec {
    type Error = Error;
    fn encode(&mut self, item: &Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use bytes::BytesMut;
    use rstest::rstest;
    use tokio::fs::File;
    use tokio_stream::StreamExt;
    use tokio_util::codec::FramedRead;
    use tokio_util::codec::{Decoder, Encoder};
    use crate::bgp::packet::message::{Message};
    use crate::bgp::packet::mock::MockTcpStream;
    use super::Codec;
    use std::env;
    use std::io::{Read, Write};
    
    #[tokio::test]
    async fn works_framedred_decode() {
        let path = env::current_dir().unwrap();
        let testdata = vec![
            ("testdata/packet/keepalive", Message::Keepalive)
        ];
        for (path, expected) in testdata {
            let mut file = std::fs::File::open(path).unwrap();
            let mut buf = vec![];
            let _ = file.read_to_end(&mut buf).unwrap();
            let mut mock_stream = MockTcpStream::new(buf);
            let codec = Codec::default();
            let mut reader = FramedRead::new(mock_stream, codec);
            let msg = reader.next().await.unwrap().unwrap();
            assert_eq!(expected, msg)
        }
    }

    #[rstest(
        input,
        expected,
        case("testdata/packet/keepalive", Message::Keepalive)
    )]
    fn works_codec_decode(input: &str, expected: Message) {
        let mut file = std::fs::File::open(input).unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        let mut codec = Codec::default();
        let mut buf = BytesMut::from(data.as_slice());
        let msg = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expected, msg);
    }
}
