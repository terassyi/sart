use std::io;

use bytes::{Buf, BufMut, BytesMut};
use serde::__private::ser::FlatMapSerializeStruct;

use crate::bgp::{error::*, family};
use crate::bgp::family::{AddressFamily, Afi, Safi};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Capability {
    MultiProtocol(AddressFamily),                       // rfc 2858 // 1
    RouteRefresh,                                       // rfc 2918 // 2
    ExtendedNextHop(Vec<(AddressFamily, u16)>),         // rfc 8950 // 5
    BGPExtendedMessage,                                 // rfc 8654 // 6
    GracefulRestart(u8, u16, Vec<(AddressFamily, u8)>), // rfc 4724 // 64
    FourOctetASNumber(u32),                             // rfc 6793 // 65
    AddPath(AddressFamily, u8),                         // rfc 7911 // 69
    EnhancedRouteRefresh,                               // rfc 7313 // 70
    Unsupported(u8, Vec<u8>),
}

impl Capability {
    pub const MULTI_PROTOCOL: u8 = 1;
    pub const ROUTE_REFRESH: u8 = 2;
    pub const EXTENDED_NEXT_HOP: u8 = 5;
    pub const BGP_EXTENDED_MESSAGE: u8 = 6;
    pub const GRACEFUL_RESTART: u8 = 64;
    pub const FOUR_OCTET_AS_NUMBER: u8 = 65;
    pub const ADD_PATH: u8 = 69;
    pub const ENHANCED_ROUTE_REFRESH: u8 = 70;

    pub const GRACEFUL_RESTART_R: u8 = 0b1000;
    pub const GRACEFUL_RESTART_B: u8 = 0b0100;

    pub fn decode(code: u8, length: u8, data: &mut BytesMut) -> Result<Self, Error> {
        if data.remaining() < length as usize {
            return Err(Error::OpenMessage(OpenMessageError::Unspecific));
        }
        match code {
            Self::MULTI_PROTOCOL => {
                let family = AddressFamily::try_from(data.get_u32())
                    .map_err(|_| Error::OpenMessage(OpenMessageError::Unspecific))?;
                Ok(Self::MultiProtocol(family))
            }
            Self::ROUTE_REFRESH => Ok(Self::RouteRefresh),
            Self::EXTENDED_NEXT_HOP => {
                let mut values: Vec<(AddressFamily, u16)> = vec![];
                let remain = data.remaining();
                while data.remaining() > remain - (length as usize) {
                    let family = AddressFamily::new(data.get_u16(), data.get_u16() as u8)
                        .map_err(|_| Error::OpenMessage(OpenMessageError::Unspecific))?;
                    values.push((family, data.get_u16()));
                }
                Ok(Self::ExtendedNextHop(values))
            }
            Self::BGP_EXTENDED_MESSAGE => Ok(Self::BGPExtendedMessage),
            Self::GRACEFUL_RESTART => {
                let remain = data.remaining();
                let flag_time = data.get_u16();
                let flags = (flag_time >> 12) as u8;
                let time = flag_time & 0x0fff;
                let mut values: Vec<(AddressFamily, u8)> = vec![];
                while data.remaining() > remain - (length as usize) {
                    let family = AddressFamily::new(data.get_u16(), data.get_u8())
                        .map_err(|_| Error::OpenMessage(OpenMessageError::Unspecific))?;
                    values.push((family, data.get_u8()));
                }
                Ok(Self::GracefulRestart(flags, time, values))
            }
            Self::FOUR_OCTET_AS_NUMBER => Ok(Self::FourOctetASNumber(data.get_u32())),
            Self::ADD_PATH => {
                let family = AddressFamily::new(data.get_u16(), data.get_u8())
                    .map_err(|_| Error::OpenMessage(OpenMessageError::Unspecific))?;
                Ok(Self::AddPath(family, data.get_u8()))
            }
            Self::ENHANCED_ROUTE_REFRESH => Ok(Self::EnhancedRouteRefresh),
            _ => {
                let mut taken_data = data.take(length as usize);
                let mut d = vec![];
                d.put(&mut taken_data);
                Ok(Self::Unsupported(code, d))
            }
        }
    }

    pub fn encode(&self, dst: &mut BytesMut) -> io::Result<()> {
        match self {
            Self::MultiProtocol(family) => {
                dst.put_u8(Self::MULTI_PROTOCOL);
                dst.put_u8(4);
                dst.put_u32(family.into());
                Ok(())
            },
            Self::RouteRefresh => {
                dst.put_u8(Self::ROUTE_REFRESH);
                dst.put_u8(0);
                Ok(())
            },
            Self::ExtendedNextHop(next_hops) => {
                dst.put_u8(Self::EXTENDED_NEXT_HOP);
                dst.put_u8((next_hops.len() * 6) as u8);
                for (family, afi) in next_hops.iter() {
                    dst.put_u16(family.afi as u16);
                    dst.put_u16(family.safi as u16);
                    dst.put_u16(*afi);
                }
                Ok(())
            },
            Self::BGPExtendedMessage => {
                dst.put_u8(Self::BGP_EXTENDED_MESSAGE);
                dst.put_u8(0);
                Ok(())
            },
            Self::GracefulRestart(flag, time, families) => {
                dst.put_u8(Self::GRACEFUL_RESTART);
                dst.put_u8(2 + (families.len() * 4) as u8);
                let a = (*flag as u16) << 12;
                dst.put_u16(((*flag as u16) << 12) + (*time & 0x0fff));
                for (family, fa) in families.iter() {
                    dst.put_u16(family.afi as u16);
                    dst.put_u8(family.safi as u8);
                    dst.put_u8(*fa);
                }
                Ok(())
            },
            Self::FourOctetASNumber(asn) => {
                dst.put_u8(Self::FOUR_OCTET_AS_NUMBER);
                dst.put_u8(4);
                dst.put_u32(*asn);
                Ok(())
            },
            Self::AddPath(family, sr) => {
                dst.put_u8(Self::ADD_PATH);
                dst.put_u8(4);
                dst.put_u16(family.afi as u16);
                dst.put_u8(family.safi as u8);
                dst.put_u8(*sr);
                Ok(())
            },
            Self::EnhancedRouteRefresh => {
                dst.put_u8(Self::ENHANCED_ROUTE_REFRESH);
                dst.put_u8(0);
                Ok(())
            },
            Self::Unsupported(code, data) => {
                dst.put_u8(*code);
                dst.put_u8(data.len() as u8);
                dst.put_slice(data);
                Ok(())
            },
            _ => Err(io::Error::from(io::ErrorKind::InvalidData)),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::MultiProtocol(_) => 4,
            Self::RouteRefresh => 0,
            Self::ExtendedNextHop(next_hops) => next_hops.len() * 6,
            Self::BGPExtendedMessage => 0,
            Self::GracefulRestart(_, _, families) => 2 + families.len() * 4,
            Self::FourOctetASNumber(_) => 4,
            Self::AddPath(_, _) => 4,
            Self::EnhancedRouteRefresh => 0,
            Self::Unsupported(_, data) => data.len(),
        }
    }
}

impl Into<u8> for Capability {
    fn into(self) -> u8 {
        match self {
            Self::MultiProtocol(_) => Self::MULTI_PROTOCOL,
            Self::RouteRefresh => Self::ROUTE_REFRESH,
            Self::ExtendedNextHop(_) => Self::EXTENDED_NEXT_HOP,
            Self::BGPExtendedMessage => Self::BGP_EXTENDED_MESSAGE,
            Self::GracefulRestart(_, _, _) => Self::GRACEFUL_RESTART,
            Self::FourOctetASNumber(_) => Self::FOUR_OCTET_AS_NUMBER,
            Self::AddPath(_, _) => Self::ADD_PATH,
            Self::EnhancedRouteRefresh => Self::ENHANCED_ROUTE_REFRESH,
            Self::Unsupported(code, _) => code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bgp::family::*;
    use bytes::BytesMut;
    use rstest::rstest;
    #[rstest(
		code,
        length,
		data,
		expected,
		case(Capability::ROUTE_REFRESH, 0, Vec::new(), Capability::RouteRefresh),
		case(Capability::BGP_EXTENDED_MESSAGE, 0, Vec::new(), Capability::BGPExtendedMessage),
		case(Capability::ENHANCED_ROUTE_REFRESH, 0, Vec::new(), Capability::EnhancedRouteRefresh),
		case(Capability::MULTI_PROTOCOL, 4, vec![0x00, 0x01, 0x00, 0x01], Capability::MultiProtocol(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast})),
		case(Capability::MULTI_PROTOCOL, 4, vec![0x00, 0x02, 0x00, 0x01], Capability::MultiProtocol(AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast})),
		case(Capability::EXTENDED_NEXT_HOP, 6, vec![0x00, 0x01, 0x00, 0x01, 0x00, 0x01], Capability::ExtendedNextHop(vec![(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x0001)])),
		case(Capability::EXTENDED_NEXT_HOP, 6, vec![0x00, 0x02, 0x00, 0x01, 0x00, 0x02], Capability::ExtendedNextHop(vec![(AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 0x0002)])),
		case(Capability::EXTENDED_NEXT_HOP, 12, vec![0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00, 0x02], Capability::ExtendedNextHop(vec![(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x0001), (AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 0x0002)])),
		case(Capability::EXTENDED_NEXT_HOP, 18, vec![0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01], Capability::ExtendedNextHop(vec![(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x0001), (AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 0x0002), (AddressFamily{afi: Afi::IPv4, safi: Safi::Multicast}, 0x0001)])),
		case(Capability::GRACEFUL_RESTART, 2, vec![0x80, 0x78], Capability::GracefulRestart(Capability::GRACEFUL_RESTART_R, 120, vec![])),
		case(Capability::GRACEFUL_RESTART, 6, vec![0xc0, 0xb4, 0x00, 0x01, 0x01, 0x01], Capability::GracefulRestart(Capability::GRACEFUL_RESTART_R + Capability::GRACEFUL_RESTART_B, 180, vec![(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x01)])),
		case(Capability::FOUR_OCTET_AS_NUMBER, 4, vec![0x00, 0x00, 0x01, 0x01], Capability::FourOctetASNumber(0x0000_0101)),
        case(Capability::ADD_PATH, 4, vec![0x00, 0x01, 0x00, 0x01, 0x01], Capability::AddPath(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x01)),
        case(Capability::ADD_PATH, 4, vec![0x00, 0x02, 0x00, 0x01, 0x02], Capability::AddPath(AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 0x02)),
		case(Capability::FOUR_OCTET_AS_NUMBER, 4, vec![0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00], Capability::FourOctetASNumber(0x0000_0101)),
        case(100, 0, vec![], Capability::Unsupported(100, Vec::new())),
	)]
    fn works_capability_decode(code: u8, length: u8, data: Vec<u8>, expected: Capability) {
        let mut buf = BytesMut::from(data.as_slice());
        match Capability::decode(code, length, &mut buf) {
            Ok(cap) => {
                assert_eq!(expected, cap);
            }
            Err(_) => assert!(false),
        }
    }
    #[rstest(
		code,
        length,
		data,
		expected,
        case(Capability::ADD_PATH, 4, vec![0x00, 0x01, 0x00], OpenMessageError::Unspecific),
    )]
    fn failed_capability_decode(code: u8, length: u8, data: Vec<u8>, expected: OpenMessageError) {
        let mut buf = BytesMut::from(data.as_slice());
        match Capability::decode(code, length, &mut buf) {
            Ok(_) => assert!(false),
            Err(e) => match e {
                Error::OpenMessage(ee) => assert_eq!(expected, ee),
                _ => assert!(false),
            },
        }
    }

    #[rstest(
        capability,
        expected,
        case(
            Capability::MultiProtocol(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}),
            vec![0x01, 0x04, 0x00, 0x01, 0x00, 0x01],
        ),
        case(
            Capability::MultiProtocol(AddressFamily{afi: Afi::IPv6, safi: Safi::Multicast}),
            vec![0x01, 0x04, 0x00, 0x02, 0x00, 0x02],
        ),
        case(Capability::RouteRefresh, vec![0x02, 0x00]),
        case(Capability::BGPExtendedMessage, vec![0x06, 0x00]),
        case(Capability::EnhancedRouteRefresh, vec![0x46, 0x00]),
        case(
            Capability::ExtendedNextHop(vec![(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x0001)]),
            vec![0x05, 0x06, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01],
        ),
		case(
            Capability::ExtendedNextHop(vec![(AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 0x0002)]),
            vec![0x05, 0x06, 0x00, 0x02, 0x00, 0x01, 0x00, 0x02],
        ),
        case(
            Capability::ExtendedNextHop(vec![(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x0001), (AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 0x0002), (AddressFamily{afi: Afi::IPv4, safi: Safi::Multicast}, 0x0001)]),
            vec![0x05, 0x12, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01],
        ),
        case(Capability::GracefulRestart(Capability::GRACEFUL_RESTART_R, 120, vec![]), vec![0x40, 0x02, 0x80, 0x78]),
        case(Capability::GracefulRestart(Capability::GRACEFUL_RESTART_R + Capability::GRACEFUL_RESTART_B, 180, vec![(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x01)]), vec![0x40, 0x06, 0xc0, 0xb4, 0x00, 0x01, 0x01, 0x01]),
		case(Capability::FourOctetASNumber(0x0000_0101), vec![0x41, 0x04, 0x00, 0x00, 0x01, 0x01]),
        case(Capability::AddPath(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x01), vec![0x45, 0x04, 0x00, 0x01, 0x01, 0x01]),
        case(Capability::AddPath(AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 0x02), vec![0x45, 0x04, 0x00, 0x02, 0x01, 0x02]),
        case(Capability::Unsupported(100, Vec::new()), vec![100, 0]),
    )]
    fn works_capability_encode(capability: Capability, expected: Vec<u8>) {
        let mut buf = BytesMut::new();
        capability.encode(&mut buf).unwrap();
        assert_eq!(expected, buf.to_vec())
    }
}
