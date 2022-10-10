use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use bytes::{Buf, BufMut, BytesMut};
use ipnet::IpAdd;
use serde_yaml::with;

use crate::bgp::error::*;
use crate::bgp::family::{AddressFamily, Afi};
use crate::bgp::packet::prefix::Prefix;

use super::capability::Capability;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Attribute {
    Origin(Base, u8),
    ASPath(Base, Vec<ASSegment>),
    NextHop(Base, Ipv4Addr),
    MultiExitDisc(Base, u32),
    LocalPref(Base, u32),
    AtomicAggregate(Base),
    Aggregator(Base, u32, IpAddr),
    Communities(Base, u32),
    MPReachNLRI(Base, AddressFamily, Vec<IpAddr>, Vec<Prefix>),
    MPUnReachNLRI(Base, AddressFamily, Vec<Prefix>),
    ExtendedCommunities(Base, u16, u64),
    AS4Path(Base, Vec<ASSegment>),
    AS4Aggregator(Base, u32, IpAddr), // todo
    Unsupported(Base, Vec<u8>),
}

impl Attribute {
    pub const ORIGIN: u8 = 1;
    pub const AS_PATH: u8 = 2;
    pub const NEXT_HOP: u8 = 3;
    pub const MULTI_EXIT_DISC: u8 = 4;
    pub const LOCAL_PREF: u8 = 5;
    pub const ATOMIC_AGGREGATE: u8 = 6;
    pub const AGGREGATOR: u8 = 7;
    pub const COMMUNITIES: u8 = 8;
    pub const MP_REACH_NLRI: u8 = 14;
    pub const MP_UNREACH_NLRI: u8 = 15;
    pub const EXTENDED_COMMUNITIES: u8 = 16;
    pub const AS4_PATH: u8 = 17;
    pub const AS4_AGGREGATOR: u8 = 18;

    pub const FLAG_OPTIONAL: u8 = 1 << 7;
    pub const FLAG_TRANSITIVE: u8 = 1 << 6;
    pub const FLAG_PARTIAL: u8 = 1 << 5;
    pub const FLAG_EXTENDED: u8 = 1 << 4;

    pub const ORIGIN_IGP: u8 = 0;
    pub const ORIGIN_EGP: u8 = 1;
    pub const ORIGIN_INCOMPLETE: u8 = 2;

    pub const AS_SET: u8 = 1;
    pub const AS_SEQUENCE: u8 = 2;

	pub fn is_transitive(&self) -> bool {
		self.get_base().is_transitive()
	}

	pub fn is_optional(&self) -> bool {
		self.get_base().is_optional()
	}

	pub fn is_partial(&self) -> bool {
		self.get_base().is_partial()
	}

	pub fn is_extended(&self) -> bool {
		self.get_base().is_extended()
	}

	fn get_base(&self) -> &Base {
        match self {
            Self::Origin(b, _) => b,
            Self::ASPath(b, _) => b,
            Self::NextHop(b, _) => b,
            Self::MultiExitDisc(b, _) => b,
            Self::LocalPref(b, _) => b,
            Self::AtomicAggregate(b) => b,
            Self::Aggregator(b, _, _) => b,
            Self::Communities(b, _) => b,
            Self::ExtendedCommunities(b, _, _) => b,
            Self::MPReachNLRI(b, _, _, _) => b,
            Self::MPUnReachNLRI(b, _, _) => b,
            Self::AS4Path(b, _) => b,
            Self::AS4Aggregator(b, _, _) => b,
            Self::Unsupported(b, _) => b,
        }
	}

    pub fn decode(data: &mut BytesMut, as4_enabled: bool, add_path_enabled: bool) -> Result<Self, Error> {
        // packet::capability will be convert to bgp capability.
        let b = Base {
            flag: data.get_u8(),
            code: data.get_u8(),
        };
        let length = if b.flag & Self::FLAG_EXTENDED != 0 {
            data.get_u16() as usize
        } else {
            data.get_u8() as usize
        };
        let remain = data.remaining();
        match b.code {
            Self::ORIGIN => Ok(Self::Origin(b, data.get_u8())),
            Self::AS_PATH => {
                let mut segments = Vec::new();
                loop {
                    if data.remaining() + length <= remain {
                        break;
                    }
                    let segment_type = data.get_u8();
                    let segment_length = data.get_u8() as usize;
                    let mut segs = Vec::with_capacity(segment_length);
                    for _ in 0..segment_length {
                        if as4_enabled {
                            segs.push(data.get_u32());
                        } else {
                            segs.push(data.get_u16() as u32);
                        }
                    }
                    segments.push(ASSegment {
                        segment_type,
                        segments: segs,
                    })
                }
                Ok(Self::ASPath(b, segments))
            }
            Self::NEXT_HOP => Ok(Self::NextHop(b, Ipv4Addr::from(data.get_u32()))),
            Self::MULTI_EXIT_DISC => Ok(Self::MultiExitDisc(b, data.get_u32())),
            Self::LOCAL_PREF => Ok(Self::LocalPref(b, data.get_u32())),
            Self::ATOMIC_AGGREGATE => Ok(Self::AtomicAggregate(b)),
            Self::AGGREGATOR => {
                let (asn, remain) = if as4_enabled { (data.get_u32(), length - 4) } else { (data.get_u16() as u32, length - 2) };
                let addr = match remain {
                    4 => IpAddr::V4(Ipv4Addr::from(data.get_u32())),
                    16 => IpAddr::V6(Ipv6Addr::from(data.get_u128())),
                    _ => return Err(Error::UpdateMessage(UpdateMessageError::OptionalAttributeError)),
                };
                Ok(Self::Aggregator(b, asn, addr))
            },
            Self::COMMUNITIES => Ok(Self::Communities(b, data.get_u32())),
            Self::EXTENDED_COMMUNITIES => {
                let val = data.get_u64();
                let typ = (val >> 48) as u16;
                let val = val & 0x0000_1111_1111_1111;
                Ok(Self::ExtendedCommunities(b, typ, val))
            }
            Self::MP_REACH_NLRI => {
                let family = AddressFamily::new(data.get_u16(), data.get_u8()).map_err(|_| {
                    Error::UpdateMessage(UpdateMessageError::OptionalAttributeError)
                })?;
                let add_path_enabled = false; // TODO: implement capability handling
                let next_hop_length = data.get_u8();
                let mut next_hops = Vec::new();
                match family.afi {
                    Afi::IPv4 => {
                        for _ in 0..(next_hop_length / 4) {
                            next_hops.push(IpAddr::V4(Ipv4Addr::from(data.get_u32())));
                        }
                    }
                    Afi::IPv6 => {
                        for _ in 0..(next_hop_length / 16) {
                            next_hops.push(IpAddr::V6(Ipv6Addr::from(data.get_u128())));
                        }
                    }
                };
                let _ = data.get_u8();
                let mut nlri = Vec::new();
                while data.remaining() > remain - length {
                    nlri.push(Prefix::decode(family, add_path_enabled, data)?)
                }
                Ok(Self::MPReachNLRI(b, family, next_hops, nlri))
            }
            Self::MP_UNREACH_NLRI => {
                let family = AddressFamily::new(data.get_u16(), data.get_u8()).map_err(|_| {
                    Error::UpdateMessage(UpdateMessageError::OptionalAttributeError)
                })?;
                let mut withdrawn_routes = Vec::new();
                while data.remaining() > remain - length {
                    withdrawn_routes.push(Prefix::decode(family, false, data)?);
                }
                Ok(Self::MPUnReachNLRI(b, family, withdrawn_routes))
            }
            Self::AS4_PATH => {
                let mut segments = Vec::new();
                while data.remaining() > remain - length {
                    let segment_type = data.get_u8();
                    let segment_length = data.get_u8() as usize;
                    let mut segs = Vec::with_capacity(segment_length);
                    for _ in 0..segment_length {
                        segs.push(data.get_u32());
                    }
                    segments.push(ASSegment {
                        segment_type,
                        segments: segs,
                    })
                }
                Ok(Self::AS4Path(b, segments))
            }
            Self::AS4_AGGREGATOR => match length {
                8 => Ok(Self::AS4Aggregator(
                    b,
                    data.get_u32(),
                    IpAddr::V4(Ipv4Addr::from(data.get_u32())),
                )),
                20 => Ok(Self::Aggregator(
                    b,
                    data.get_u32(),
                    IpAddr::V6(Ipv6Addr::from(data.get_u128())),
                )),
                _ => Err(Error::UpdateMessage(
                    UpdateMessageError::OptionalAttributeError,
                )),
            },
            _ => {
                let mut taken = data.take(length as usize);
                let mut d = vec![];
                d.put(&mut taken);
                Ok(Self::Unsupported(b, d))
            }
        }
    }

	pub fn encode(&self, dst: &mut BytesMut, as4_enabled: bool, add_path_enabled: bool) -> io::Result<()> {
		dst.put_u16(self.get_base().into());
		if self.is_extended() {
             dst.put_u16(self.len(as4_enabled, add_path_enabled) as u16)
		} else { dst.put_u8(self.len(as4_enabled, add_path_enabled) as u8) }
		match self {
			Self::Origin(_, val) => {
                dst.put_u8(*val);
				Ok(())
			},
			Self::ASPath(_, segments) => {
                for seg in segments.iter() {
                    dst.put_u8(seg.segment_type);
                    dst.put_u8(seg.segments.len() as u8);
                    for asn in seg.segments.iter() {
                        if as4_enabled { dst.put_u32(*asn) } else { dst.put_u16(*asn as u16) }
                    }
                }
				Ok(())
			},
			Self::NextHop(_, next) => {
                dst.put_slice(&next.octets());
				Ok(())
			},
			Self::MultiExitDisc(_, val) => {
                dst.put_u32(*val);
				Ok(())
			},
			Self::LocalPref(_, pref) => {
                dst.put_u32(*pref);
				Ok(())
			},
			Self::AtomicAggregate(_) => {
				Ok(())
			},
			Self::Aggregator(_, val, addr) => {
                if as4_enabled { dst.put_u32(*val) } else {dst.put_u16(*val as u16) }
                match addr {
                    IpAddr::V4(a) => {
                        let oct = a.octets();
                        dst.put_slice(&oct);
                    },
                    IpAddr::V6(a) => {
                        let oct = a.octets();
                        dst.put_slice(&oct);
                    },
                };
				Ok(())
			},
			Self::Communities(_, val) => {
                dst.put_u32(*val);
				Ok(())
			},
			Self::ExtendedCommunities(_, val1, val2) => {
                let typ = (*val1 as u64) << 48;
                dst.put_u64(typ + *val2);
				Ok(())
			},
			Self::MPReachNLRI(_, family, addrs, prefixes) => {
                dst.put_u16(family.afi as u16);
                dst.put_u8(family.safi as u8);
                dst.put_u8(match family.afi {
                    Afi::IPv4 => (addrs.len() * 4) as u8,
                    Afi::IPv6 => (addrs.len() * 16) as u8,
                });
                for addr in addrs.iter() {
                    match addr {
                        IpAddr::V4(a) => {
                            let oct = a.octets();
                            dst.put_slice(&oct);
                        },
                        IpAddr::V6(a) => {
                            let oct = a.octets();
                            dst.put_slice(&oct);
                        }
                    }
                }
                dst.put_u8(0);
                for prefix in prefixes.iter() {
                    prefix.encode(dst)?;
                }
				Ok(())
			},
			Self::MPUnReachNLRI(_, family, prefixes) => {
                dst.put_u16(family.afi as u16);
                dst.put_u8(family.safi as u8);
                for prefix in prefixes.iter() {
                    prefix.encode(dst)?;
                }
				Ok(())
			},
			Self::AS4Path(_, segments) => {
                for seg in segments.iter() {
                    dst.put_u8(seg.segment_type);
                    dst.put_u8(seg.segments.len() as u8);
                    for asn in seg.segments.iter() {
                        dst.put_u32(*asn);
                    }
                }
				Ok(())
			},
			Self::AS4Aggregator(_, val, addr) => {
                dst.put_u32(*val);
                match addr {
                    IpAddr::V4(a) => {
                        let oct = a.octets();
                        dst.put_slice(&oct);
                    },
                    IpAddr::V6(a) => {
                        let oct = a.octets();
                        dst.put_slice(&oct);
                    },
                };
				Ok(())
			},
			Self::Unsupported(_, data) => {
                dst.put_slice(data);
				Ok(())
			}
			_ => Err(io::Error::from(io::ErrorKind::InvalidData)),
		}
	}

	fn len(&self, as4_enabled: bool, add_path_enabled: bool) -> usize {
        match self {
            Self::Origin(_, _) => 1,
            Self::ASPath(_, segments) => {
                segments.iter().fold(0, |a, b| {
                    a + 2 + b.segments.iter().fold(0, |aa: usize, _| aa + if as4_enabled { 4 } else { 2 })
                })
			},
            Self::NextHop(_, _) => 4,
            Self::MultiExitDisc(_, _) => 4,
            Self::LocalPref(_, _) => 4,
            Self::AtomicAggregate(_) => 0,
            Self::Aggregator(_, _, addr) => {
                if as4_enabled {
                    match addr {
                        IpAddr::V4(_) => 8,
                        IpAddr::V6(_) => 20,
                    }
                } else {
                    match addr {
                        IpAddr::V4(_) => 6,
                        IpAddr::V6(_) => 18,
                    }
                }
			},
            Self::Communities(_, _) => 4,
            Self::ExtendedCommunities(_, _, _) => 8,
            Self::MPReachNLRI(_, family, next_hops, prefixes) => {
                let length = next_hops.iter().fold(5, |l, next_hop| {
                    match next_hop {
                        IpAddr::V4(_) => l + 4,
                        IpAddr::V6(_) => l + 16,
                    }
                });
                prefixes.iter().fold(length, |l, prefix| l + prefix.len())
			},
            Self::MPUnReachNLRI(_, _, prefixes) => {
                prefixes.iter().fold(3, |l, prefix| l + prefix.len())
			},
            Self::AS4Path(_, segments) => {
                segments.iter().fold(0, |a, b| {
                    a + 2 + b.segments.iter().fold(0, |aa: usize, _| aa + 4)
                })
			},
            Self::AS4Aggregator(_, _, addr) => {
                match addr {
                    IpAddr::V4(_) => 8,
                    IpAddr::V6(_) => 20,
                }
			},
            Self::Unsupported(_, data) => data.len(),
        }
	}
}

impl Into<u8> for Attribute {
    fn into(self) -> u8 {
        match self {
            Self::Origin(_, _) => Self::ORIGIN,
            Self::ASPath(_, _) => Self::AS_PATH,
            Self::NextHop(_, _) => Self::NEXT_HOP,
            Self::MultiExitDisc(_, _) => Self::MULTI_EXIT_DISC,
            Self::LocalPref(_, _) => Self::LOCAL_PREF,
            Self::AtomicAggregate(_) => Self::ATOMIC_AGGREGATE,
            Self::Aggregator(_, _, _) => Self::AGGREGATOR,
            Self::Communities(_, _) => Self::COMMUNITIES,
            Self::ExtendedCommunities(_, _, _) => Self::EXTENDED_COMMUNITIES,
            Self::MPReachNLRI(_, _, _, _) => Self::MP_REACH_NLRI,
            Self::MPUnReachNLRI(_, _, _) => Self::MP_UNREACH_NLRI,
            Self::AS4Path(_, _) => Self::AS4_PATH,
            Self::AS4Aggregator(_, _, _) => Self::AS4_AGGREGATOR,
            Self::Unsupported(b, _) => b.code,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Base {
    flag: u8,
    code: u8,
}

impl Base {
    pub fn new(flag: u8, code: u8) -> Self {
        Self { flag, code }
    }

	fn is_extended(&self) -> bool {
		(self.flag & Attribute::FLAG_EXTENDED) != 0
	}

	fn is_transitive(&self) -> bool {
		(self.flag & Attribute::FLAG_TRANSITIVE) != 0
	}

	fn is_optional(&self) -> bool {
		(self.flag & Attribute::FLAG_OPTIONAL) != 0
	}

	fn is_partial(&self) -> bool {
		(self.flag & Attribute::FLAG_PARTIAL) != 0
	}
}

impl<'a> Into<u16> for &'a Base {
	fn into(self) -> u16 {
		((self.flag as u16) << 8) + (self.code as u16)
	}
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ASSegment {
    pub segment_type: u8,
    pub segments: Vec<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::BytesMut;
    use std::str::FromStr;
    use ipnet::{IpNet, Ipv4Net, Ipv6Net};
    use crate::bgp::family::{AddressFamily, Afi, Safi};
    use rstest::rstest;

    #[rstest(
		input,
        as4_enabled,
        add_path_enabled,
		expected,
		case(vec![0x40, 0x01, 0x01, 0x00], false, false, Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_IGP)),
		case(vec![0x40, 0x01, 0x01, 0x01], false, false, Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_EGP)),
		case(vec![0x40, 0x01, 0x01, 0x02], false, false, Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_INCOMPLETE)),
	)]
    fn works_attribute_decode_origin(input: Vec<u8>, as4_enabled: bool, add_path_enabled: bool, expected: Attribute) {
        let mut buf = BytesMut::from(input.as_slice());
        match Attribute::decode(&mut buf, as4_enabled, add_path_enabled) {
            Ok(origin) => assert_eq!(expected, origin),
            Err(_) => assert!(false),
        }
    }

    #[rstest(
		input,
        as4_enabled,
        add_path_enabled,
		expected,
		case(vec![0x40, 0x02, 0x04, 0x02, 0x01, 0xfd, 0xe9], false, false, Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![65001]}])),
		case(vec![0x40, 0x02, 0x06, 0x02, 0x02, 0x5b, 0xa0, 0x5b, 0xa0], false, false, Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![23456, 23456]}])),
		case(vec![0x40, 0x02, 0x0a, 0x02, 0x01, 0x00, 0x1e, 0x01, 0x02, 0x00, 0x0a, 0x00, 0x14], false, false, Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, 
			vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![30]}, ASSegment{segment_type: Attribute::AS_SET, segments: vec![10 ,20]}],
		)),
        case(vec![0x40, 0x02, 0x06, 0x02, 0x01, 0xfd, 0xe8, 0xfd, 0xe8], true, false, Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![4259905000]}])),
        case(vec![0x40, 0x02, 0x0e, 0x02, 0x03, 0x00, 0x0a, 0x00,  0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03], true, false, Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![655361, 2, 3]}])),
	)]
    fn works_attribute_decode_as_path(input: Vec<u8>, as4_enabled: bool, add_path_enabled: bool, expected: Attribute) {
        let mut buf = BytesMut::from(input.as_slice());
        match Attribute::decode(&mut buf, as4_enabled, false) {
            Ok(as_path) => assert_eq!(expected, as_path),
            Err(_) => assert!(false),
        }
    }

    #[rstest(
        input,
        as4_enabled,
        expected,
        case(vec![0xc0, 0x07, 0x08, 0x00, 0x00, 0x00, 0x64, 0x01, 0x01, 0x01, 0x01], true, Attribute::Aggregator(Base{flag: Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE, code: Attribute::AGGREGATOR}, 100, IpAddr::V4(Ipv4Addr::new(1,1,1,1)))),
        case(vec![0xc0, 0x07, 0x06, 0x5b, 0xa0, 0x02, 0x02, 0x02, 0x02], false, Attribute::Aggregator(Base{flag: Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE, code: Attribute::AGGREGATOR}, 23456, IpAddr::V4(Ipv4Addr::new(2,2,2,2)))),
        case(vec![0xc0, 0x12, 0x08, 0x00, 0x09, 0xff, 0x60, 0x02,  0x02, 0x02, 0x02], false, Attribute::AS4Aggregator(Base{flag: Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE, code: Attribute::AS4_AGGREGATOR}, 655200, IpAddr::V4(Ipv4Addr::new(2,2,2,2)))),
    )]
    fn works_attribute_decode_aggregator(input: Vec<u8>, as4_enabled: bool, expected: Attribute) {
        let mut buf = BytesMut::from(input.as_slice());
        match Attribute::decode(&mut buf, as4_enabled, false) {
            Ok(as_path) => assert_eq!(expected, as_path),
            Err(_) => assert!(false),
        }

    }

    #[rstest(
        input,
        expected,
        case(vec![0x40, 0x03, 0x04, 0xac, 0x10, 0x01, 0x01], Attribute::NextHop(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::NEXT_HOP}, Ipv4Addr::new(172, 16, 1, 1))),
        case(vec![0x40, 0x03, 0x04, 0x0a, 0x00, 0x00, 0x02], Attribute::NextHop(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::NEXT_HOP}, Ipv4Addr::new(10, 0, 0, 2))),
        case(vec![0x80, 0x04, 0x04, 0x00, 0x00, 0x00, 0x00], Attribute::MultiExitDisc(Base{flag: Attribute::FLAG_OPTIONAL, code: Attribute::MULTI_EXIT_DISC}, 0)),
        case(vec![0x40, 0x06, 0x00], Attribute::AtomicAggregate(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ATOMIC_AGGREGATE})),
        case(
            vec![0x80, 0x0e, 0x40, 0x00, 0x02, 0x01, 0x20, 0x20, 0x01, 0x0d, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xfe, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0,
                0x01, 0x0b, 0xff, 0xfe, 0x7e, 0x00, 0x00, 0x00, 0x40, 0x20, 0x01, 0x0d, 0xb8, 0x00, 0x01, 0x00,
                0x02, 0x40, 0x20, 0x01, 0x0d, 0xb8, 0x00, 0x01, 0x00, 0x01, 0x40, 0x20, 0x01, 0x0d, 0xb8, 0x00,
                0x01, 0x00, 0x00],
            Attribute::MPReachNLRI(
                Base{flag: Attribute::FLAG_OPTIONAL, code: Attribute::MP_REACH_NLRI}, 
                AddressFamily{afi: Afi::IPv6, safi: Safi::Unicast}, 
                vec![IpAddr::V6(Ipv6Addr::from_str("2001:db8::1").unwrap()), IpAddr::V6(Ipv6Addr::from_str("fe80::c001:bff:fe7e:0").unwrap())], 
                vec![Prefix::new("2001:db8:1:2::/64".parse().unwrap(), None), Prefix::new("2001:db8:1:1::/64".parse().unwrap(), None), Prefix::new("2001:db8:1::/64".parse().unwrap(), None)]),
        ),
    )]
    fn works_attribute_decode_other(input: Vec<u8>, expected: Attribute) {
        let mut buf = BytesMut::from(input.as_slice());
        match Attribute::decode(&mut buf, false, false) {
            Ok(as_path) => assert_eq!(expected, as_path),
            Err(_) => assert!(false),
        }
    }

    #[rstest(
        attr,
        as4_enabled,
        add_path_enabled,
        expected,
        case(Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_IGP), false, false, vec![0x40, 0x01, 0x01, 0x00]),
        case(Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_EGP), false, false, vec![0x40, 0x01, 0x01, 0x01]),
        case(Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![65001]}]), false, false, vec![0x40, 0x02, 0x04, 0x02, 0x01, 0xfd, 0xe9]),
        case(Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE + Attribute::FLAG_EXTENDED, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![23456, 100]}]), false, false, vec![0x50, 0x02, 0x00, 0x06, 0x02, 0x02, 0x5b, 0xa0, 0x00, 0x64]),
        case(Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![655361, 2]}]), true, false, vec![0x40, 0x02, 0x0a, 0x02, 0x02, 0x00, 0x0a, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02]),
        case(Attribute::NextHop(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::NEXT_HOP}, Ipv4Addr::new(10, 0, 0, 1)), false, false, vec![0x40, 0x03, 0x04, 0x0a, 0x00, 0x00, 0x01]),
        case(Attribute::MultiExitDisc(Base{flag: Attribute::FLAG_OPTIONAL, code: Attribute::MULTI_EXIT_DISC}, 0), false, false, vec![0x80, 0x04, 0x04, 0x00, 0x00, 0x00, 0x00]),
        case(Attribute::AS4Path(Base{flag: Attribute::FLAG_TRANSITIVE + Attribute::FLAG_EXTENDED + Attribute::FLAG_OPTIONAL, code: Attribute::AS4_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![655200, 100]}]), false, false, vec![0xd0, 0x11, 0x00, 0x0a, 0x02, 0x02, 0x00, 0x09, 0xff, 0x60, 0x00, 0x00, 0x00, 0x64]),
        case(Attribute::MPReachNLRI(Base{flag: Attribute::FLAG_OPTIONAL, code: Attribute::MP_REACH_NLRI}, 
            AddressFamily{ afi: Afi::IPv6, safi: Safi::Unicast}, 
            vec![IpAddr::V6(Ipv6Addr::from_str("2001:db8::1").unwrap()), IpAddr::V6(Ipv6Addr::from_str("fe80::c001:bff:fe7e:0").unwrap())], 
            vec![Prefix::new("2001:db8:1:2::/64".parse().unwrap(), None), Prefix::new("2001:db8:1:1::/64".parse().unwrap(), None), Prefix::new("2001:db8:1::/64".parse().unwrap(), None)],
        ), 
        false, 
        false, 
        vec![0x80, 0x0e, 0x40, 0x00, 0x02, 0x01, 0x20, 0x20, 0x01, 0x0d, 0xb8, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xfe, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0,
            0x01, 0x0b, 0xff, 0xfe, 0x7e, 0x00, 0x00, 0x00, 0x40, 0x20, 0x01, 0x0d, 0xb8, 0x00, 0x01, 0x00,
            0x02, 0x40, 0x20, 0x01, 0x0d, 0xb8, 0x00, 0x01, 0x00, 0x01, 0x40, 0x20, 0x01, 0x0d, 0xb8, 0x00,
            0x01, 0x00, 0x00]),
        case(Attribute::AtomicAggregate(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ATOMIC_AGGREGATE}), false, false, vec![0x40, 0x06, 0x00]),
        case(Attribute::Aggregator(Base{flag: Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL, code: Attribute::AGGREGATOR}, 23456, IpAddr::V4(Ipv4Addr::new(2,2,2,2))), false, false, vec![0xc0, 0x07, 0x06, 0x5b, 0xa0, 0x02, 0x02, 0x02, 0x02]),
        case(Attribute::AS4Path(Base{flag: Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE + Attribute::FLAG_EXTENDED, code: Attribute::AS4_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![655200]}]), false, false, vec![0xd0, 0x11, 0x00, 0x06, 0x02, 0x01, 0x00, 0x09, 0xff, 0x60]),
        case(Attribute::AS4Aggregator(Base{flag: Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL, code: Attribute::AS4_AGGREGATOR}, 655200, IpAddr::V4(Ipv4Addr::new(2,2,2,2))), false, false, vec![0xc0, 0x12, 0x08, 0x00, 0x09, 0xff, 0x60, 0x02,  0x02, 0x02, 0x02]),
    )]
    fn works_attribute_encode(attr: Attribute, as4_enabled: bool, add_path_enabled: bool, expected: Vec<u8>) {
        let mut buf = BytesMut::new();
        attr.encode(&mut buf, as4_enabled, add_path_enabled).unwrap();
        assert_eq!(expected, buf.to_vec())
    }
}
