use bytes::{Buf, BytesMut};

use crate::bgp::error::*;
use crate::bgp::family::AddressFamily;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Capability {
    MultiProtocol(AddressFamily),                       // rfc 2858 // 1
    RouteRefresh,                                       // rfc 2918 // 2
    ExtendedNextHop(Vec<(AddressFamily, u16)>),         // rfc 8950 // 5
    BGPExtendedMessage,                                 // rfc 8654 // 6
    GracefulRestart(u8, u16, Vec<(AddressFamily, u8)>), // rfc 4274 // 64
    FourOctetASNumber(u32),                             // rfc 6793 // 65
    AddPath(AddressFamily, u8),                         // rfc 7911 // 69
    EnhancedRouteRefresh,                               // rfc 7313 // 70
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

	pub const GRACEFUL_RESTART_R: u8 = 0x10;
	pub const GRACEFUL_RESTART_B: u8 = 0x01;

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
                loop {
                    if data.remaining() <= remain - (length as usize) {
                        break;
                    }
                    let family = AddressFamily::try_from(data.get_u32())
                        .map_err(|_| Error::OpenMessage(OpenMessageError::Unspecific))?;
                    values.push((family, data.get_u16()));
                }
                Ok(Self::ExtendedNextHop(values))
            }
            Self::BGP_EXTENDED_MESSAGE => Ok(Self::BGPExtendedMessage),
            Self::GRACEFUL_RESTART => {
                let remain = data.remaining();
                let flags = data.get_u8();
                let time = data.get_u16();
                let mut values: Vec<(AddressFamily, u8)> = vec![];
                loop {
                    if data.remaining() <= remain - (length as usize) {
                        break;
                    }
                    let family = AddressFamily::try_from(data.get_u32())
                        .map_err(|_| Error::OpenMessage(OpenMessageError::Unspecific))?;
                    values.push((family, data.get_u8()));
                }
                Ok(Self::GracefulRestart(flags, time, values))
            }
            Self::FOUR_OCTET_AS_NUMBER => Ok(Self::FourOctetASNumber(data.get_u32())),
            Self::ADD_PATH => {
                let family = AddressFamily::try_from(data.get_u32())
                    .map_err(|_| Error::OpenMessage(OpenMessageError::Unspecific))?;
                Ok(Self::AddPath(family, data.get_u8()))
            }
            Self::ENHANCED_ROUTE_REFRESH => Ok(Self::EnhancedRouteRefresh),
            _ => Err(Error::OpenMessage(
                OpenMessageError::UnsupportedOptionalParameter,
            )),
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
		case(Capability::GRACEFUL_RESTART, 8, vec![0x10, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x01], Capability::GracefulRestart(Capability::GRACEFUL_RESTART_R, 0x0100, vec![(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x01)])),
		case(Capability::GRACEFUL_RESTART, 8, vec![0x11, 0xff, 0xff, 0x00, 0x01, 0x00, 0x01, 0x01], Capability::GracefulRestart(Capability::GRACEFUL_RESTART_R + Capability::GRACEFUL_RESTART_B, 0xffff, vec![(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x01)])),
		case(Capability::GRACEFUL_RESTART, 13, vec![0x11, 0xff, 0xff, 0x00, 0x01, 0x00, 0x01, 0x01, 0x00, 0x02, 0x00, 0x01, 0x02], Capability::GracefulRestart(Capability::GRACEFUL_RESTART_R + Capability::GRACEFUL_RESTART_B, 0xffff, vec![(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x01), (AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 0x02)])),
		case(Capability::FOUR_OCTET_AS_NUMBER, 4, vec![0x00, 0x00, 0x01, 0x01], Capability::FourOctetASNumber(0x0000_0101)),
        case(Capability::ADD_PATH, 4, vec![0x00, 0x01, 0x00, 0x01, 0x01], Capability::AddPath(AddressFamily{afi: Afi::IPv4, safi: Safi::Unicast}, 0x01)),
        case(Capability::ADD_PATH, 4, vec![0x00, 0x02, 0x00, 0x01, 0x02], Capability::AddPath(AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 0x02)),
		case(Capability::FOUR_OCTET_AS_NUMBER, 4, vec![0x00, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00], Capability::FourOctetASNumber(0x0000_0101)),
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
        case(100, 0, vec![], OpenMessageError::UnsupportedOptionalParameter),
        case(Capability::ADD_PATH, 4, vec![0x00, 0x01, 0x00], OpenMessageError::Unspecific),
    )]
    fn failed_capability_decode(code: u8, length: u8, data: Vec<u8>, expected: OpenMessageError) {
        let mut buf = BytesMut::from(data.as_slice());
        match Capability::decode(code, length, &mut buf) {
            Ok(_) => assert!(false),
            Err(e) => match e {
                Error::OpenMessage(ee) => assert_eq!(expected, ee),
                _ => assert!(false)
            },
        }
    }

}
