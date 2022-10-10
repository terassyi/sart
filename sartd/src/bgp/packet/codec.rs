use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::sync::{Arc, RwLock};

use bytes::{Buf, BytesMut, BufMut};
use serde_yaml::with;
use tokio_util::codec::{Decoder, Encoder};

use crate::bgp::family::{AddressFamily, Afi, Safi};
use crate::bgp::packet::attribute::Attribute;
use crate::bgp::packet::message::{Message, MessageType, NotificationCode, NotificationSubCode};
use crate::bgp::packet::prefix::Prefix;
use crate::bgp::{capability, error::*};

use super::capability::Capability;
use super::prefix;

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
            capabilities: Arc::new(RwLock::new(HashMap::new())),
        }
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
                let capability_length = src.get_u8();
                if capability_length != src.remaining() as u8 {
                    return Err(Error::MessageHeader(MessageHeaderError::BadMessageLength {
                        length: src.len() as u16,
                    }));
                }
                loop {
                    if src.remaining() == 0 {
                        break;
                    }
                    let option_type = src.get_u8();
                    let _option_length = src.get_u8();
                    match option_type {
                        Message::OPTION_TYPE_CAPABILITIES => {
                            let cap = Capability::decode(src.get_u8(), src.get_u8(), src)?;
                            capabilities.push(cap);
                        }
                        _ => {
                            return Err(Error::OpenMessage(
                                OpenMessageError::UnsupportedOptionalParameter,
                            ))
                        }
                    }
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
                        attributes.push(Attribute::decode(src, false, false)?); // TODO
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
        dst.put_u128(Message::MARKER);
        dst.put_u16(Message::HEADER_LENGTH);
        let msg_type: MessageType = item.into();
        dst.put_u8(msg_type as u8);
        match item {
            Message::Open { version, as_num, hold_time, identifier, capabilities } => {
                dst.put_u8(*version);
                dst.put_u16(if *as_num > 65535 { Message::AS_TRANS } else { *as_num  } as u16);
                dst.put_u16(*hold_time);
                dst.put_slice(&identifier.octets());
                dst.put_u8(capabilities.iter().fold(0, |l, cap| l + 2 + cap.len()) as u8);
                for cap in capabilities.iter() {
                    cap.encode(dst)?;
                }
            },
            Message::Update { withdrawn_routes, attributes, nlri } => {
                if let Some(withdrawn_routes) = withdrawn_routes {
                    for route in withdrawn_routes.iter() {
                        route.encode(dst)?;
                    }
                }
                if let Some(attributes) = attributes {
                    for attr in attributes.iter() {
                        attr.encode(dst, false, false)?; // TODO: flag will be set based on enabled capabilities
                    }
                }
                if let Some(nlri) = nlri {
                    for prefix in nlri.iter() {
                        prefix.encode(dst)?;
                    }
                }
            },
            Message::Keepalive => {},
            Message::Notification { code, subcode, data } => {
                dst.put_u8(*code as u8);
                if let Some(subcode) = subcode {
                    dst.put_u8(*subcode as u8);
                } else {}
                if data.len() > 0 {
                    dst.put_slice(data);
                }
            },
            Message::RouteRefresh { family } => {
                dst.put_u32(family.into());
            },
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Codec;
    use crate::bgp::family::{AddressFamily, Afi, Safi};
    use crate::bgp::packet::attribute::{ASSegment, Attribute, Base};
    use crate::bgp::packet::capability::Capability;
    use crate::bgp::packet::message::{Message, NotificationCode, NotificationSubCode};
    use crate::bgp::packet::mock::MockTcpStream;
    use crate::bgp::packet::prefix::Prefix;
    use bytes::BytesMut;
    use ipnet::{IpNet, Ipv4Net, Ipv6Net};
    use rstest::rstest;
    use std::env;
    use std::io::{Read, Write};
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::fs::File;
    use tokio_stream::StreamExt;
    use tokio_util::codec::FramedRead;
    use tokio_util::codec::{Decoder, Encoder};

    #[tokio::test]
    async fn works_framedred_decode() {
        let path = env::current_dir().unwrap();
        let testdata = vec![("testdata/messages/keepalive", Message::Keepalive)];
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
        case("testdata/messages/keepalive", Message::Keepalive),
        case("testdata/messages/notification-bad-as-peer", Message::Notification {
            code: NotificationCode::OpenMessage,
            subcode: Some(NotificationSubCode::BadPeerAS),
            data: vec![0xfe, 0xb0],
        }),
        case("testdata/messages/open-2bytes-asn", Message::Open {
            version: 4,
            as_num: 65100,
            hold_time: 180,
            identifier: Ipv4Addr::new(10, 10, 3, 1),
            capabilities: vec![
                Capability::MultiProtocol(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast }),
                Capability::Unsupported(0x80, Vec::new()), // Unsupported Route Refresh Cisco
                Capability::RouteRefresh,
            ] }),
        case("testdata/messages/open-4bytes-asn", Message::Open { 
            version: 4,
            as_num: Message::AS_TRANS,
            hold_time: 180,
            identifier: Ipv4Addr::new(40, 0, 0, 1),
            capabilities: vec![
                Capability::MultiProtocol(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast }),
                Capability::Unsupported(0x80, Vec::new()), // Unsupported Route Refresh Cisco
                Capability::RouteRefresh,
                Capability::Unsupported(131, vec![0x00]), // Unsupported Multisession BPGP Cisco
                Capability::FourOctetASNumber(2621441),
            ]
        }),
        case("testdata/messages/open-graceful-restart", Message::Open {
            version: 4,
            as_num: 100,
            hold_time: 180,
            identifier: Ipv4Addr::new(1, 1, 1, 1),
            capabilities: vec![
                Capability::MultiProtocol(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast }),
                Capability::Unsupported(0x80, Vec::new()), // Unsupported Route Refresh Cisco
                Capability::RouteRefresh,
                Capability::FourOctetASNumber(100),
                Capability::AddPath(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast}, 1),
                Capability::Unsupported(0x49, vec![0x02, 0x52, 0x30, 0x00]),
                Capability::GracefulRestart(Capability::GRACEFUL_RESTART_R, 120, vec![]),
            ]
        }),
        case("testdata/messages/open-ipv6", Message::Open {
            version: 4,
            as_num: 65002,
            hold_time: 180,
            identifier: Ipv4Addr::new(2, 2, 2, 2),
            capabilities: vec![
                Capability::MultiProtocol(AddressFamily{ afi: Afi::IPv6, safi: Safi::Unicast }),
                Capability::Unsupported(0x80, Vec::new()), // Unsupported Route Refresh Cisco
                Capability::RouteRefresh,
            ]
        }),
        case("testdata/messages/route-refresh", Message::RouteRefresh { family: AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast} }),
        case("testdata/messages/update-as-set", Message::Update {
            withdrawn_routes: None,
            attributes: Some(vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_INCOMPLETE),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![30]}, ASSegment{ segment_type: Attribute::AS_SET, segments: vec![10, 20]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(10, 0, 0, 9)),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                Attribute::Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE, Attribute::AGGREGATOR), 30, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))),
            ]),
            nlri: Some(vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(172, 16, 0, 0), 21).unwrap()), None),
            ]),
        }),
        case("testdata/messages/update-as4-path", Message::Update {
            withdrawn_routes: None,
            attributes: Some(vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::AS4Path(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL, Attribute::AS4_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![655361, 2621441]}]),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![23456, 23456]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(172, 16, 3, 1)),
            ]),
            nlri: Some(vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(40, 0, 0, 0), 8).unwrap()), None),
            ]),
        }),
        case("testdata/messages/update-as4-path-aggregator", Message::Update {
            withdrawn_routes: None,
            attributes: Some(vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_EXTENDED, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![100, 23456]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(10, 0, 0, 2)),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                Attribute::Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE + Attribute::FLAG_PARTIAL, Attribute::AGGREGATOR), 23456, IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))),
                Attribute::AS4Path(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL + Attribute::FLAG_PARTIAL + Attribute::FLAG_EXTENDED, Attribute::AS4_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![655200]}]),
                Attribute::AS4Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE + Attribute::FLAG_PARTIAL, Attribute::AS4_AGGREGATOR), 655200, IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))),
            ]),
            nlri: Some(vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(192, 168, 0, 0), 16).unwrap()), None),
            ]),
        }),
        case("testdata/messages/update-mp-reach-nlri", Message::Update {
            withdrawn_routes: None,
            attributes: Some(vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![65001]}]),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                Attribute::MPReachNLRI(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MP_REACH_NLRI), AddressFamily{ afi: Afi::IPv6, safi: Safi::Unicast }, vec![IpAddr::V6("2001:db8::1".parse().unwrap()), IpAddr::V6("fe80::c001:bff:fe7e:0".parse().unwrap())], vec![Prefix::new("2001:db8:1:2::/64".parse().unwrap(), None), Prefix::new("2001:db8:1:1::/64".parse().unwrap(), None), Prefix::new("2001:db8:1::/64".parse().unwrap(), None)]),
            ]),
            nlri: None,
        }),
        case("testdata/messages/update-nlri", Message::Update {
            withdrawn_routes: None,
            attributes: Some(vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![65100]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(1, 1, 1, 1)),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
            ]),
            nlri: Some(vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 3, 0), 24).unwrap()), None),
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 2, 0), 24).unwrap()), None),
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 1, 0), 24).unwrap()), None),
            ]),
        }),
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
