use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::sync::{Arc, RwLock};

use byteorder::{NetworkEndian, WriteBytesExt};
use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use crate::bgp::capability::CapabilitySet;
use crate::bgp::family::{AddressFamily, Afi, Safi};
use crate::bgp::packet::attribute::Attribute;
use crate::bgp::packet::message::{Message, MessageType, NotificationCode, NotificationSubCode};
use crate::bgp::packet::prefix::Prefix;
use crate::bgp::{capability, error::*};

use super::capability::Capability;

#[derive(Debug)]
pub(crate) struct Codec {
    family: AddressFamily,
    capabilities: Arc<RwLock<capability::CapabilitySet>>,
    as4_enabled: bool,
    path_id_enabled: bool,
}

impl Codec {
    pub fn new(as4_enabled: bool, path_id_enabled: bool) -> Self {
        Self {
            family: AddressFamily {
                afi: Afi::IPv4,
                safi: Safi::Unicast,
            },
            capabilities: Arc::new(RwLock::new(CapabilitySet::with_empty())),
            as4_enabled,
            path_id_enabled,
        }
    }

    pub fn default() -> Self {
        Self {
            family: AddressFamily {
                afi: Afi::IPv4,
                safi: Safi::Unicast,
            },
            capabilities: Arc::new(RwLock::new(CapabilitySet::with_empty())),
            as4_enabled: true,
            path_id_enabled: false,
        }
    }
}

impl Decoder for Codec {
    type Item = Vec<Message>;
    type Error = Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }
        let mut messages = Vec::new();
        while src.remaining() > 0 {
            let msg = decode_msg(src, &self.family, self.as4_enabled, self.path_id_enabled)?;
            messages.push(msg);
        }
        Ok(Some(messages))
    }
}

fn decode_msg(
    src: &mut BytesMut,
    family: &AddressFamily,
    as4_enabled: bool,
    add_path_enabled: bool,
) -> Result<Message, Error> {
    let buf_length = src.len();
    let marker = src.get_u128();
    let message_length = src.get_u16();

    let message_tail = buf_length - message_length as usize;

    if marker != Message::MARKER || message_length < Message::HEADER_LENGTH {
        return Err(Error::MessageHeader(MessageHeaderError::BadMessageLength {
            length: message_length,
        }));
    }
    let message_type = MessageType::try_from(src.get_u8())?;
    let message: Message = match message_type {
        MessageType::Open => {
            let version = src.get_u8();
            if version != Message::VERSION {
                return Err(Error::OpenMessage(
                    OpenMessageError::UnsupportedVersionNumber,
                ));
            }
            let asn = src.get_u16();
            if asn == 0 {
                return Err(Error::OpenMessage(OpenMessageError::BadPeerAS));
            }
            let hold_time = src.get_u16();
            let router_id = Ipv4Addr::from(src.get_u32());

            let mut capabilities = vec![];
            let optional_parameters_length = src.get_u8();
            if optional_parameters_length > src.remaining() as u8 {
                return Err(Error::MessageHeader(MessageHeaderError::BadMessageLength {
                    length: src.len() as u16,
                }));
            }
            while src.remaining() > message_tail {
                let option_type = src.get_u8();
                let option_length = src.get_u8();
                match option_type {
                    Message::OPTION_TYPE_CAPABILITIES => {
                        let mut l = 0;
                        while option_length > l {
                            let code = src.get_u8();
                            let length = src.get_u8();
                            l += length + 2;
                            let cap = Capability::decode(code, length, src)?;
                            capabilities.push(cap);
                        }
                    }
                    _ => {
                        return Err(Error::OpenMessage(
                            OpenMessageError::UnsupportedOptionalParameter,
                        ))
                    }
                }
            }

            Message::Open {
                version,
                as_num: asn as u32,
                hold_time,
                identifier: router_id,
                capabilities,
            }
        }
        MessageType::Update => {
            let withdrawn_routes_length = src.get_u16() as usize;
            let mut withdrawn_routes = Vec::new();
            if withdrawn_routes_length > 0 {
                let remain = src.remaining();
                while src.remaining() > remain - withdrawn_routes_length {
                    withdrawn_routes.push(Prefix::decode(family, false, src)?);
                }
            };

            let total_path_attribute_length = src.get_u16() as usize;
            let mut attributes = Vec::new();
            if total_path_attribute_length > 0 {
                let remain = src.remaining();
                let mut attribute_set = HashSet::new();
                while src.remaining() > remain - (total_path_attribute_length) {
                    let attr = Attribute::decode(src, as4_enabled, false)?;
                    // check well-known mandatory attributes exists
                    if !attribute_set.insert(attr.code()) {
                        return Err(Error::UpdateMessage(
                            UpdateMessageError::MalformedAttributeList,
                        ));
                    }
                    attributes.push(attr);
                }
                if !attribute_set.contains(&Attribute::ORIGIN) {
                    return Err(Error::UpdateMessage(
                        UpdateMessageError::MissingWellKnownAttribute(Attribute::ORIGIN),
                    ));
                }
                if !attribute_set.contains(&Attribute::AS_PATH) {
                    return Err(Error::UpdateMessage(
                        UpdateMessageError::MissingWellKnownAttribute(Attribute::AS_PATH),
                    ));
                }
                if !attribute_set.contains(&Attribute::NEXT_HOP)
                    && !attribute_set.contains(&Attribute::MP_REACH_NLRI)
                {
                    return Err(Error::UpdateMessage(
                        UpdateMessageError::MissingWellKnownAttribute(Attribute::NEXT_HOP),
                    ));
                }
            };

            let mut nlri = Vec::new();
            if src.remaining() > message_tail {
                while src.remaining() > message_tail {
                    nlri.push(Prefix::decode(family, false, src)?)
                }
            };

            Message::Update {
                withdrawn_routes,
                attributes,
                nlri,
            }
        }
        MessageType::Keepalive => Message::Keepalive {},
        MessageType::Notification => {
            let code = NotificationCode::try_from(src.get_u8())?;
            let subcode = NotificationSubCode::try_from_with_code(src.get_u8(), code)?;
            let mut data = vec![];
            let data_length = message_length - Message::HEADER_LENGTH - 2;
            let taken = src.take(data_length as usize);
            data.put(taken);
            Message::Notification {
                code,
                subcode,
                data,
            }
        }
        MessageType::RouteRefresh => {
            let family = AddressFamily::try_from(src.get_u32())
                .map_err(|_| Error::RouteRefreshMessageError)?;
            Message::RouteRefresh { family }
        }
    };
    Ok(message)
}

impl Encoder<Message> for Codec {
    type Error = Error;
    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let msg_head = dst.len();
        dst.put_u128(Message::MARKER);
        let header_length_head = dst.len();
        dst.put_u16(Message::HEADER_LENGTH);
        let msg_type: MessageType = (&item).into();
        dst.put_u8(msg_type as u8);
        match item {
            Message::Open {
                version,
                as_num,
                hold_time,
                identifier,
                capabilities,
            } => {
                dst.put_u8(version);
                dst.put_u16(if as_num > 65535 {
                    Message::AS_TRANS
                } else {
                    as_num
                } as u16);
                dst.put_u16(hold_time);
                dst.put_slice(&identifier.octets());
                dst.put_u8(capabilities.iter().fold(0, |l, cap| l + 2 + 2 + cap.len()) as u8);
                for cap in capabilities.iter() {
                    dst.put_u8(Message::OPTION_TYPE_CAPABILITIES);
                    dst.put_u8(cap.len() as u8 + 2);
                    cap.encode(dst)?;
                }
            }
            Message::Update {
                withdrawn_routes,
                attributes,
                nlri,
            } => {
                if !withdrawn_routes.is_empty() {
                    dst.put_u16(withdrawn_routes.iter().fold(0, |l, p| l + p.len() as u16));
                    for route in withdrawn_routes.iter() {
                        route.encode(dst)?;
                    }
                } else {
                    dst.put_u16(0);
                }
                if !attributes.is_empty() {
                    dst.put_u16(attributes.iter().fold(0, |l, a| {
                        let ll = if a.is_extended() { 4 } else { 3 };
                        l + ll + a.len(self.as4_enabled, false) as u16
                    }));
                    for attr in attributes.iter() {
                        attr.encode(dst, self.as4_enabled, false)?; // TODO: flag will be set based on enabled capabilities
                    }
                } else {
                    dst.put_u16(0);
                }
                if !nlri.is_empty() {
                    for prefix in nlri.iter() {
                        prefix.encode(dst)?;
                    }
                }
            }
            Message::Keepalive => {}
            Message::Notification {
                code,
                subcode,
                data,
            } => {
                dst.put_u8(code as u8);
                if let Some(subcode) = subcode {
                    dst.put_u8(subcode.into());
                } else {
                    dst.put_u8(0);
                }
                if !data.is_empty() {
                    dst.put_slice(&data);
                }
            }
            Message::RouteRefresh { family } => {
                dst.put_u32(family.into());
            }
        }
        let msg_tail = dst.len();
        (&mut dst.as_mut()[header_length_head..])
            .write_u16::<NetworkEndian>((msg_tail - msg_head) as u16)?;
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
    use ipnet::{IpNet, Ipv4Net};
    use rstest::rstest;
    use std::io::Read;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio_stream::StreamExt;
    use tokio_util::codec::FramedRead;
    use tokio_util::codec::{Decoder, Encoder};

    #[tokio::test]
    async fn works_framedread_decode() {
        let testdata = vec![("testdata/messages/keepalive", Message::Keepalive)];
        for (path, expected) in testdata {
            let mut file = std::fs::File::open(path).unwrap();
            let mut buf = vec![];
            let _ = file.read_to_end(&mut buf).unwrap();
            let mock_stream = MockTcpStream::new(buf);
            let codec = Codec::default();
            let mut reader = FramedRead::new(mock_stream, codec);
            let msg = reader.next().await.unwrap().unwrap();
            assert_eq!(expected, msg[0])
        }
    }

    #[rstest(
        input,
        as4_enabled,
        path_id_enabled,
        expected,
        case("testdata/messages/keepalive", true, false, Message::Keepalive),
        case("testdata/messages/notification-bad-as-peer", true, false, Message::Notification {
            code: NotificationCode::OpenMessage,
            subcode: Some(NotificationSubCode::BadPeerAS),
            data: vec![0xfe, 0xb0],
        }),
        case("testdata/messages/open-2bytes-asn", false, false, Message::Open {
            version: 4,
            as_num: 65100,
            hold_time: 180,
            identifier: Ipv4Addr::new(10, 10, 3, 1),
            capabilities: vec![
                Capability::MultiProtocol(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast }),
                Capability::Unsupported(0x80, Vec::new()), // Unsupported Route Refresh Cisco
                Capability::RouteRefresh,
            ] }),
        case("testdata/messages/open-4bytes-asn", true, false, Message::Open {
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
        case("testdata/messages/open-graceful-restart", true, false, Message::Open {
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
        case("testdata/messages/open-ipv6", false, false, Message::Open {
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
        case("testdata/messages/open-optional-parameters", true, false, Message::Open {
            version: 4,
            as_num: 200,
            hold_time: 90,
            identifier: Ipv4Addr::new(2, 2, 2, 2),
            capabilities: vec![
                Capability::RouteRefresh,
                Capability::MultiProtocol(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast }),
                Capability::FourOctetASNumber(200),
                Capability::ExtendedNextHop(vec![(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast }, 2)]),
            ],
        }),
        case("testdata/messages/route-refresh", true, false, Message::RouteRefresh { family: AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast} }),
        case("testdata/messages/update-as-set", false, false, Message::Update {
            withdrawn_routes: Vec::new(),
            attributes: vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_INCOMPLETE),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![30]}, ASSegment{ segment_type: Attribute::AS_SET, segments: vec![10, 20]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(10, 0, 0, 9)),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                Attribute::Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE, Attribute::AGGREGATOR), 30, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))),
            ],
            nlri: vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(172, 16, 0, 0), 21).unwrap()), None),
            ],
        }),
        case("testdata/messages/update-as4-path", false, false, Message::Update {
            withdrawn_routes: Vec::new(),
            attributes: vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::AS4Path(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL, Attribute::AS4_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![655361, 2621441]}]),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![23456, 23456]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(172, 16, 3, 1)),
            ],
            nlri: vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(40, 0, 0, 0), 8).unwrap()), None),
            ],
        }),
        case("testdata/messages/update-as4-path-aggregator", false, false, Message::Update {
            withdrawn_routes: Vec::new(),
            attributes: vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_EXTENDED, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![100, 23456]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(10, 0, 0, 2)),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                Attribute::Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE + Attribute::FLAG_PARTIAL, Attribute::AGGREGATOR), 23456, IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))),
                Attribute::AS4Path(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL + Attribute::FLAG_PARTIAL + Attribute::FLAG_EXTENDED, Attribute::AS4_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![655200]}]),
                Attribute::AS4Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE + Attribute::FLAG_PARTIAL, Attribute::AS4_AGGREGATOR), 655200, IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))),
            ],
            nlri: vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(192, 168, 0, 0), 16).unwrap()), None),
            ],
        }),
        case("testdata/messages/update-mp-reach-nlri", false, false, Message::Update {
            withdrawn_routes: Vec::new(),
            attributes: vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![65001]}]),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                Attribute::MPReachNLRI(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MP_REACH_NLRI), AddressFamily{ afi: Afi::IPv6, safi: Safi::Unicast }, vec![IpAddr::V6("2001:db8::1".parse().unwrap()), IpAddr::V6("fe80::c001:bff:fe7e:0".parse().unwrap())], vec![Prefix::new("2001:db8:1:2::/64".parse().unwrap(), None), Prefix::new("2001:db8:1:1::/64".parse().unwrap(), None), Prefix::new("2001:db8:1::/64".parse().unwrap(), None)]),
            ],
            nlri: Vec::new(),
        }),
        case("testdata/messages/update-nlri", false, false, Message::Update {
            withdrawn_routes: Vec::new(),
            attributes: vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![65100]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(1, 1, 1, 1)),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
            ],
            nlri: vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 3, 0), 24).unwrap()), None),
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 2, 0), 24).unwrap()), None),
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 1, 0), 24).unwrap()), None),
            ],
        }),
    )]
    fn works_codec_decode_single_message(
        input: &str,
        as4_enabled: bool,
        path_id_enabled: bool,
        expected: Message,
    ) {
        let mut file = std::fs::File::open(input).unwrap();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        let mut codec = Codec::new(as4_enabled, path_id_enabled);
        let mut buf = BytesMut::from(data.as_slice());
        let msg = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expected, msg[0]);
    }

    #[rstest(
        paths,
        as4_enabled,
        path_id_enabled,
        expected,
        case(vec!["testdata/messages/open-2bytes-asn", "testdata/messages/keepalive"], false, false,
            vec![
                Message::Open {
                    version: 4,
                    as_num: 65100,
                    hold_time: 180,
                    identifier: Ipv4Addr::new(10, 10, 3, 1),
                    capabilities: vec![
                        Capability::MultiProtocol(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast }),
                        Capability::Unsupported(0x80, Vec::new()), // Unsupported Route Refresh Cisco
                        Capability::RouteRefresh,
                    ]
                },
                Message::Keepalive,
            ],
        ),
        case(vec!["testdata/messages/update-as4-path-aggregator", "testdata/messages/update-nlri"], false, false,
            vec![
                Message::Update {
                    withdrawn_routes: Vec::new(),
                    attributes: vec![
                        Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                        Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_EXTENDED, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![100, 23456]}]),
                        Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(10, 0, 0, 2)),
                        Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                        Attribute::Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE + Attribute::FLAG_PARTIAL, Attribute::AGGREGATOR), 23456, IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))),
                        Attribute::AS4Path(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL + Attribute::FLAG_PARTIAL + Attribute::FLAG_EXTENDED, Attribute::AS4_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![655200]}]),
                        Attribute::AS4Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE + Attribute::FLAG_PARTIAL, Attribute::AS4_AGGREGATOR), 655200, IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))),
                    ],
                    nlri: vec![
                        Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(192, 168, 0, 0), 16).unwrap()), None),
                    ],
                },
                Message::Update {
                    withdrawn_routes: Vec::new(),
                    attributes: vec![
                        Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                        Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![65100]}]),
                        Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(1, 1, 1, 1)),
                        Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                    ],
                    nlri: vec![
                        Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 3, 0), 24).unwrap()), None),
                        Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 2, 0), 24).unwrap()), None),
                        Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 1, 0), 24).unwrap()), None),
                    ],
                },
            ],
        ),
    )]
    fn works_codec_decode_multiple_messages(
        paths: Vec<&str>,
        as4_enabled: bool,
        path_id_enabled: bool,
        expected: Vec<Message>,
    ) {
        let mut data = Vec::new();
        for path in paths.iter() {
            let mut d = Vec::new();
            let mut file = std::fs::File::open(path).unwrap();
            file.read_to_end(&mut d).unwrap();
            data.append(&mut d);
        }
        let mut codec = Codec::new(as4_enabled, path_id_enabled);
        let mut buf = BytesMut::from(data.as_slice());
        let msgs = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(expected, msgs);
    }

    // #[rstest(
    //     input,
    //     as4_enabled,
    //     path_id_enabled,
    //     expected,
    //     case("testdata/messages/open-bad-message-length", false, false, Message::Open {
    //         version: 4,
    //         as_num: 300,
    //         hold_time: 90,
    //         identifier: Ipv4Addr::new(3, 3, 3, 3),
    //         capabilities: vec![
    //             Capability::RouteRefresh,
    //             Capability::MultiProtocol(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast }),
    //             Capability::FourOctetASNumber(300),
    //         ]
    //     }),
    // )]
    // fn fail_codec_decode(
    //     input: &str,
    //     as4_enabled: bool,
    //     path_id_enabled: bool,
    //     expected: Message,
    // ) {
    //     let mut file = std::fs::File::open(input).unwrap();
    //     let mut data = Vec::new();
    //     file.read_to_end(&mut data).unwrap();
    //     let mut codec = Codec::new(as4_enabled, path_id_enabled);
    //     let mut buf = BytesMut::from(data.as_slice());
    //     let msg = codec.decode(&mut buf).unwrap().unwrap();
    //     assert_eq!(expected, msg);
    // }

    #[rstest(
        path,
        as4_enabled,
        path_id_enabled,
        msg,
        case("testdata/messages/keepalive", true, false, Message::Keepalive),
        case("testdata/messages/notification-bad-as-peer", true, false, Message::Notification {
            code: NotificationCode::OpenMessage,
            subcode: Some(NotificationSubCode::BadPeerAS),
            data: vec![0xfe, 0xb0],
        }),
        case("testdata/messages/open-2bytes-asn", false, false, Message::Open {
            version: 4,
            as_num: 65100,
            hold_time: 180,
            identifier: Ipv4Addr::new(10, 10, 3, 1),
            capabilities: vec![
                Capability::MultiProtocol(AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast }),
                Capability::Unsupported(0x80, Vec::new()), // Unsupported Route Refresh Cisco
                Capability::RouteRefresh,
            ] }),
        case("testdata/messages/open-4bytes-asn", true, false, Message::Open {
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
        case("testdata/messages/open-graceful-restart", true, false, Message::Open {
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
        case("testdata/messages/open-ipv6", false, false, Message::Open {
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
        case("testdata/messages/route-refresh", true, false, Message::RouteRefresh { family: AddressFamily{ afi: Afi::IPv4, safi: Safi::Unicast} }),
        case("testdata/messages/update-as-set", false, false, Message::Update {
            withdrawn_routes: Vec::new(),
            attributes: vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_INCOMPLETE),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![30]}, ASSegment{ segment_type: Attribute::AS_SET, segments: vec![10, 20]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(10, 0, 0, 9)),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                Attribute::Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE, Attribute::AGGREGATOR), 30, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))),
            ],
            nlri: vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(172, 16, 0, 0), 21).unwrap()), None),
            ],
        }),
        case("testdata/messages/update-as4-path", false, false, Message::Update {
            withdrawn_routes: Vec::new(),
            attributes: vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::AS4Path(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL, Attribute::AS4_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![655361, 2621441]}]),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![23456, 23456]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(172, 16, 3, 1)),
            ],
            nlri: vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(40, 0, 0, 0), 8).unwrap()), None),
            ],
        }),
        case("testdata/messages/update-as4-path-aggregator", false, false, Message::Update {
            withdrawn_routes: Vec::new(),
            attributes: vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_EXTENDED, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![100, 23456]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(10, 0, 0, 2)),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                Attribute::Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE + Attribute::FLAG_PARTIAL, Attribute::AGGREGATOR), 23456, IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))),
                Attribute::AS4Path(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL + Attribute::FLAG_PARTIAL + Attribute::FLAG_EXTENDED, Attribute::AS4_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![655200]}]),
                Attribute::AS4Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE + Attribute::FLAG_PARTIAL, Attribute::AS4_AGGREGATOR), 655200, IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2))),
            ],
            nlri: vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(192, 168, 0, 0), 16).unwrap()), None),
            ],
        }),
        case("testdata/messages/update-mp-reach-nlri", false, false, Message::Update {
            withdrawn_routes: Vec::new(),
            attributes: vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![65001]}]),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
                Attribute::MPReachNLRI(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MP_REACH_NLRI), AddressFamily{ afi: Afi::IPv6, safi: Safi::Unicast }, vec![IpAddr::V6("2001:db8::1".parse().unwrap()), IpAddr::V6("fe80::c001:bff:fe7e:0".parse().unwrap())], vec![Prefix::new("2001:db8:1:2::/64".parse().unwrap(), None), Prefix::new("2001:db8:1:1::/64".parse().unwrap(), None), Prefix::new("2001:db8:1::/64".parse().unwrap(), None)]),
            ],
            nlri: Vec::new(),
        }),
        case("testdata/messages/update-nlri", false, false, Message::Update {
            withdrawn_routes: Vec::new(),
            attributes: vec![
                Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
                Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![65100]}]),
                Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(1, 1, 1, 1)),
                Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
            ],
            nlri: vec![
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 3, 0), 24).unwrap()), None),
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 2, 0), 24).unwrap()), None),
                Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 1, 0), 24).unwrap()), None),
            ],
        }),
    )]
    fn works_codec_encode(path: &str, as4_enabled: bool, path_id_enabled: bool, msg: Message) {
        let mut file = std::fs::File::open(path).unwrap();
        let mut expected_data = Vec::new();
        file.read_to_end(&mut expected_data).unwrap();
        let mut codec = Codec::new(as4_enabled, path_id_enabled);
        let b: Vec<u8> = Vec::new();
        let mut buf = BytesMut::from(b.as_slice());
        codec.encode(msg, &mut buf).unwrap();
        assert_eq!(expected_data, buf.to_vec());
    }
}
