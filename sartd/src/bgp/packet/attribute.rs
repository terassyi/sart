use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use bytes::{BytesMut, Buf, BufMut};
use ipnet::IpAdd;
use serde_yaml::with;

use crate::bgp::family::{AddressFamily, Afi};
use crate::bgp::packet::prefix::Prefix;
use crate::bgp::error::*;

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

	pub fn decode(data: &mut BytesMut) -> Result<Self, Error> { // packet::capability will be convert to bgp capability.
		let b = Base{ flag: data.get_u8(), code: data.get_u8() };
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
						segs.push(data.get_u16() as u32);
					};
					segments.push(ASSegment { segment_type, segments: segs })
				}
				Ok(Self::ASPath(b, segments))
			} 
			Self::NEXT_HOP => Ok(Self::NextHop(b, Ipv4Addr::from(data.get_u32()))),
			Self::MULTI_EXIT_DISC => Ok(Self::MultiExitDisc(b, data.get_u32())),
			Self::LOCAL_PREF => Ok(Self::LocalPref(b, data.get_u32())),
			Self::ATOMIC_AGGREGATE => Ok(Self::AtomicAggregate(b)),
			Self::AGGREGATOR => {
				match length {
					6 => Ok(Self::Aggregator(b, data.get_u16() as u32, IpAddr::V4(Ipv4Addr::from(data.get_u32())))),
					18 => Ok(Self::Aggregator(b, data.get_u16() as u32, IpAddr::V6(Ipv6Addr::from(data.get_u128())))),
					_ => Err(Error::UpdateMessage(UpdateMessageError::OptionalAttributeError)),
				}
			},
			Self::COMMUNITIES => Ok(Self::Communities(b, data.get_u32())),
			Self::EXTENDED_COMMUNITIES => {
				let val = data.get_u64();
				let typ = (val >> 48) as u16;
				let val = val & 0x0000_1111_1111_1111;
				Ok(Self::ExtendedCommunities(b, typ, val))
			},
			Self::MP_REACH_NLRI => {
				let family = AddressFamily::try_from(data.get_u32())
					.map_err(|_| Error::UpdateMessage(UpdateMessageError::OptionalAttributeError))?;
				let add_path_enabled = false; // TODO: implement capability handling
				let next_hop_length = data.get_u8();
				let mut next_hops = Vec::new();
				match family.afi {
					Afi::IPv4 => {
						for _ in 0..(next_hop_length / 4) {
							next_hops.push(IpAddr::V4(Ipv4Addr::from(data.get_u32())));
						}
					},
					Afi::IPv6 => {
						for _ in 0..(next_hop_length / 16) {
							next_hops.push(IpAddr::V6(Ipv6Addr::from(data.get_u128())));
						}
					},
				};
				let _ = data.get_u8();
				let mut nlri = Vec::new();
				loop {
					if data.remaining() + length <= remain {
						break;
					}
					nlri.push(Prefix::decode(family, add_path_enabled, data)?)
				}
				Ok(Self::MPReachNLRI(b, family, next_hops, nlri))
			},
			Self::MP_UNREACH_NLRI => {
				let family = AddressFamily::try_from(data.get_u32())
					.map_err(|_| Error::UpdateMessage(UpdateMessageError::OptionalAttributeError))?;
				let mut withdrawn_routes = Vec::new();
				loop {
					if data.remaining() + length <= remain {
						break;
					}
					withdrawn_routes.push(Prefix::decode(family, false, data)?);
				}
				Ok(Self::MPUnReachNLRI(b, family, withdrawn_routes))
			},
			Self::AS4_PATH => {
				let mut segments = Vec::new();
				loop {
					if data.remaining() + length <= remain {
						break;
					}
					let segment_type = data.get_u8();
					let segment_length = data.get_u8() as usize;
					let mut segs = Vec::with_capacity(segment_length);
					for _ in 0..segment_length {
						segs.push(data.get_u32());
					};
					segments.push(ASSegment { segment_type, segments: segs })
				}
				Ok(Self::ASPath(b, segments))
			},
			Self::AGGREGATOR => {
				match length {
					8 => Ok(Self::Aggregator(b, data.get_u32(), IpAddr::V4(Ipv4Addr::from(data.get_u32())))),
					20 => Ok(Self::Aggregator(b, data.get_u32(), IpAddr::V6(Ipv6Addr::from(data.get_u128())))),
					_ => Err(Error::UpdateMessage(UpdateMessageError::OptionalAttributeError)),
				}
			},
			_ => {
				let mut taken = data.take(length as usize);
				let mut d = vec![];
				d.put(&mut taken);
				Ok(Self::Unsupported(b, d))
			},
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ASSegment {
	segment_type: u8,
	segments: Vec<u32>,
}

#[cfg(test)]
mod tests {
	use super::*;
	use bytes::BytesMut;
	use rstest::rstest;

	#[rstest(
		input,
		expected,
		case(vec![0x40, 0x01, 0x01, 0x00], Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_IGP)),
		case(vec![0x40, 0x01, 0x01, 0x01], Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_EGP)),
		case(vec![0x40, 0x01, 0x01, 0x02], Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_INCOMPLETE)),
	)]
	fn works_attribute_decode_origin(input: Vec<u8>, expected: Attribute) {
		let mut buf = BytesMut::from(input.as_slice());
		match Attribute::decode(&mut buf) {
			Ok(origin) => assert_eq!(expected, origin),
			Err(_) => assert!(false),
		}
	}

	#[rstest(
		input,
		expected,
		case(vec![0x40, 0x02, 0x04, 0x02, 0x01, 0xfd, 0xe9], Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![65001]}])),
		case(vec![0x40, 0x02, 0x06, 0x02, 0x02, 0x5b, 0xa0, 0x5b, 0xa0], Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![23456, 23456]}])),
		case(vec![0x40, 0x02, 0x0a, 0x02, 0x01, 0x00, 0x1e, 0x01, 0x02, 0x00, 0x0a, 0x00, 0x14], Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, 
			vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![30]}, ASSegment{segment_type: Attribute::AS_SET, segments: vec![10 ,20]}],
		))
	)]
	fn works_attribute_decode_as_path(input: Vec<u8>, expected: Attribute) {
		let mut buf = BytesMut::from(input.as_slice());
		match Attribute::decode(&mut buf) {
			Ok(as_path) => assert_eq!(expected, as_path),
			Err(_) => assert!(false),
		}
	}
}
