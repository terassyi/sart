use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use bytes::{Buf, BufMut, BytesMut};
use prost::Message;

use super::prefix::Prefix;
use crate::error::*;
use crate::family::{AddressFamily, Afi};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Attribute {
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

    pub const AS_TRANS: u32 = 23456;
    pub const AS_2BYTES_LIMIT: u32 = u16::MAX as u32;

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

    pub fn is_recognized(&self) -> bool {
        !matches!(self, Self::Unsupported(_, _))
    }

    pub fn set_partial(&mut self) {
        self.get_base_mut().flag += Attribute::FLAG_PARTIAL;
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

    fn get_base_mut(&mut self) -> &mut Base {
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

    pub fn new_origin(val: u8) -> Result<Attribute, Error> {
        match val {
            Attribute::ORIGIN_IGP | Attribute::ORIGIN_EGP | Attribute::ORIGIN_INCOMPLETE => {
                Ok(Attribute::Origin(
                    Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN),
                    val,
                ))
            }
            _ => Err(Error::UpdateMessage(
                UpdateMessageError::InvalidOriginAttribute(val),
            )),
        }
    }

    pub fn new_nexthop(addr: Ipv4Addr) -> Attribute {
        Attribute::NextHop(
            Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP),
            addr,
        )
    }

    pub fn new_as_path(
        as_sequence: Vec<u32>,
        as_set: Vec<u32>,
        extend: bool,
    ) -> Result<Attribute, Error> {
        let flag = if extend {
            Attribute::FLAG_TRANSITIVE + Attribute::FLAG_EXTENDED
        } else {
            Attribute::FLAG_TRANSITIVE
        };
        let segments = match as_set.is_empty() {
            false => vec![
                ASSegment::new(Attribute::AS_SEQUENCE, as_sequence),
                ASSegment::new(Attribute::AS_SET, as_set),
            ],
            true => vec![ASSegment::new(Attribute::AS_SEQUENCE, as_sequence)],
        };
        Ok(Attribute::ASPath(
            Base::new(flag, Attribute::AS_PATH),
            segments,
        ))
    }

    pub fn new_as4_path(
        as_sequence: Vec<u32>,
        as_set: Vec<u32>,
        extend: bool,
    ) -> Result<Attribute, Error> {
        let flag = if extend {
            Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL + Attribute::FLAG_EXTENDED
        } else {
            Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL
        };
        let segments = match as_set.is_empty() {
            false => vec![
                ASSegment::new(Attribute::AS_SEQUENCE, as_sequence),
                ASSegment::new(Attribute::AS_SET, as_set),
            ],
            true => vec![ASSegment::new(Attribute::AS_SEQUENCE, as_sequence)],
        };
        Ok(Attribute::AS4Path(
            Base::new(flag, Attribute::AS4_PATH),
            segments,
        ))
    }

    pub fn new_mp_reach_nlri(
        family: AddressFamily,
        nexthops: Vec<IpAddr>,
        prefixes: Vec<Prefix>,
    ) -> Result<Attribute, Error> {
        Ok(Attribute::MPReachNLRI(
            Base::new(Attribute::FLAG_OPTIONAL, Attribute::MP_REACH_NLRI),
            family,
            nexthops,
            prefixes,
        ))
    }

    pub fn new_mp_unreach_nlri(
        family: AddressFamily,
        prefixes: Vec<Prefix>,
    ) -> Result<Attribute, Error> {
        Ok(Attribute::MPUnReachNLRI(
            Base::new(Attribute::FLAG_OPTIONAL, Attribute::MP_UNREACH_NLRI),
            family,
            prefixes,
        ))
    }

    pub fn new_local_pref(val: u32) -> Result<Attribute, Error> {
        Ok(Attribute::LocalPref(
            Base::new(Attribute::FLAG_TRANSITIVE, Attribute::LOCAL_PREF),
            val,
        ))
    }

    pub fn new_med(val: u32) -> Result<Attribute, Error> {
        Ok(Attribute::MultiExitDisc(
            Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC),
            val,
        ))
    }

    pub fn code(&self) -> u8 {
        self.get_base().code
    }

    pub fn decode(
        data: &mut BytesMut,
        as4_enabled: bool,
        _add_path_enabled: bool,
    ) -> Result<Self, Error> {
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
        if !b.validate_attribute_flag() {
            return Err(Error::UpdateMessage(
                UpdateMessageError::AttributeFlagsError {
                    code: b.code,
                    length,
                    value: b.flag,
                },
            ));
        }
        let remain = data.remaining();
        match b.code {
            Self::ORIGIN => {
                let val = data.get_u8();
                if val > 2 {
                    return Err(Error::UpdateMessage(
                        UpdateMessageError::InvalidOriginAttribute(val),
                    ));
                }
                Ok(Self::Origin(b, val))
            }
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
                // if segments.is_empty() {
                // return Err(Error::UpdateMessage(UpdateMessageError::MalformedASPath));
                // }
                Ok(Self::ASPath(b, segments))
            }
            Self::NEXT_HOP => Ok(Self::NextHop(b, Ipv4Addr::from(data.get_u32()))), // validate whether next_hop value is correct
            Self::MULTI_EXIT_DISC => Ok(Self::MultiExitDisc(b, data.get_u32())),
            Self::LOCAL_PREF => Ok(Self::LocalPref(b, data.get_u32())),
            Self::ATOMIC_AGGREGATE => Ok(Self::AtomicAggregate(b)),
            Self::AGGREGATOR => {
                let (asn, remain) = if as4_enabled {
                    (data.get_u32(), length - 4)
                } else {
                    (data.get_u16() as u32, length - 2)
                };
                let addr = match remain {
                    4 => IpAddr::V4(Ipv4Addr::from(data.get_u32())),
                    16 => IpAddr::V6(Ipv6Addr::from(data.get_u128())),
                    _ => {
                        return Err(Error::UpdateMessage(
                            UpdateMessageError::OptionalAttributeError,
                        ))
                    }
                };
                Ok(Self::Aggregator(b, asn, addr))
            }
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
                    nlri.push(Prefix::decode(&family, add_path_enabled, data)?)
                }
                Ok(Self::MPReachNLRI(b, family, next_hops, nlri))
            }
            Self::MP_UNREACH_NLRI => {
                let family = AddressFamily::new(data.get_u16(), data.get_u8()).map_err(|_| {
                    Error::UpdateMessage(UpdateMessageError::OptionalAttributeError)
                })?;
                let mut withdrawn_routes = Vec::new();
                while data.remaining() > remain - length {
                    withdrawn_routes.push(Prefix::decode(&family, false, data)?);
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
                if !b.is_optional() {
                    return Err(Error::UpdateMessage(
                        UpdateMessageError::UnrecognizedWellknownAttribute(b.code),
                    ));
                }
                let mut taken = data.take(length);
                let mut d = vec![];
                d.put(&mut taken);
                Ok(Self::Unsupported(b, d))
            }
        }
    }

    pub fn encode(
        &self,
        dst: &mut BytesMut,
        as4_enabled: bool,
        add_path_enabled: bool,
    ) -> io::Result<()> {
        dst.put_u16(self.get_base().into());
        if self.is_extended() {
            dst.put_u16(self.len(as4_enabled, add_path_enabled) as u16)
        } else {
            dst.put_u8(self.len(as4_enabled, add_path_enabled) as u8)
        }
        match self {
            Self::Origin(_, val) => {
                dst.put_u8(*val);
                Ok(())
            }
            Self::ASPath(_, segments) => {
                if !segments.iter().any(|s| s.len() > 0) {
                    return Ok(());
                }
                for seg in segments.iter() {
                    dst.put_u8(seg.segment_type);
                    dst.put_u8(seg.segments.len() as u8);
                    for asn in seg.segments.iter() {
                        if as4_enabled {
                            dst.put_u32(*asn)
                        } else {
                            dst.put_u16(*asn as u16)
                        }
                    }
                }
                Ok(())
            }
            Self::NextHop(_, next) => {
                dst.put_slice(&next.octets());
                Ok(())
            }
            Self::MultiExitDisc(_, val) => {
                dst.put_u32(*val);
                Ok(())
            }
            Self::LocalPref(_, pref) => {
                dst.put_u32(*pref);
                Ok(())
            }
            Self::AtomicAggregate(_) => Ok(()),
            Self::Aggregator(_, val, addr) => {
                if as4_enabled {
                    dst.put_u32(*val)
                } else {
                    dst.put_u16(*val as u16)
                }
                match addr {
                    IpAddr::V4(a) => {
                        let oct = a.octets();
                        dst.put_slice(&oct);
                    }
                    IpAddr::V6(a) => {
                        let oct = a.octets();
                        dst.put_slice(&oct);
                    }
                };
                Ok(())
            }
            Self::Communities(_, val) => {
                dst.put_u32(*val);
                Ok(())
            }
            Self::ExtendedCommunities(_, val1, val2) => {
                let typ = (*val1 as u64) << 48;
                dst.put_u64(typ + *val2);
                Ok(())
            }
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
                        }
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
            }
            Self::MPUnReachNLRI(_, family, prefixes) => {
                dst.put_u16(family.afi as u16);
                dst.put_u8(family.safi as u8);
                for prefix in prefixes.iter() {
                    prefix.encode(dst)?;
                }
                Ok(())
            }
            Self::AS4Path(_, segments) => {
                if !segments.iter().any(|s| s.len() > 0) {
                    return Ok(());
                }
                for seg in segments.iter() {
                    dst.put_u8(seg.segment_type);
                    dst.put_u8(seg.segments.len() as u8);
                    for asn in seg.segments.iter() {
                        dst.put_u32(*asn);
                    }
                }
                Ok(())
            }
            Self::AS4Aggregator(_, val, addr) => {
                dst.put_u32(*val);
                match addr {
                    IpAddr::V4(a) => {
                        let oct = a.octets();
                        dst.put_slice(&oct);
                    }
                    IpAddr::V6(a) => {
                        let oct = a.octets();
                        dst.put_slice(&oct);
                    }
                };
                Ok(())
            }
            Self::Unsupported(_, data) => {
                dst.put_slice(data);
                Ok(())
            }
        }
    }

    pub fn len(&self, as4_enabled: bool, _add_path_enabled: bool) -> usize {
        match self {
            Self::Origin(_, _) => 1,
            Self::ASPath(_, segments) => segments.iter().fold(0, |a, b| {
                if !segments.iter().any(|s| s.len() > 0) {
                    return 0;
                }
                a + 2
                    + b.segments
                        .iter()
                        .fold(0, |aa: usize, _| aa + if as4_enabled { 4 } else { 2 })
            }),
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
            }
            Self::Communities(_, _) => 4,
            Self::ExtendedCommunities(_, _, _) => 8,
            Self::MPReachNLRI(_, _, next_hops, prefixes) => {
                let length = next_hops.iter().fold(5, |l, next_hop| match next_hop {
                    IpAddr::V4(_) => l + 4,
                    IpAddr::V6(_) => l + 16,
                });
                prefixes.iter().fold(length, |l, prefix| l + prefix.len())
            }
            Self::MPUnReachNLRI(_, _, prefixes) => {
                prefixes.iter().fold(3, |l, prefix| l + prefix.len())
            }
            Self::AS4Path(_, segments) => segments.iter().fold(0, |a, b| {
                if !segments.iter().any(|s| s.len() > 0) {
                    return 0;
                }
                a + 2 + b.segments.iter().fold(0, |aa: usize, _| aa + 4)
            }),
            Self::AS4Aggregator(_, _, addr) => match addr {
                IpAddr::V4(_) => 8,
                IpAddr::V6(_) => 20,
            },
            Self::Unsupported(_, data) => data.len(),
        }
    }

    // this is not enough to express all attribute's key and value pair
    pub fn from_str_pair(key: &str, value: &str) -> Result<Attribute, Error> {
        if key == "origin" {
            Attribute::new_origin(value.parse().unwrap())
        } else if key == "local_pref" {
            Attribute::new_local_pref(value.parse().unwrap())
        } else if key == "med" {
            Attribute::new_med(value.parse().unwrap())
        } else {
            Err(Error::Config(ConfigError::InvalidArgument(
                "failed to parse key value pair".to_string(),
            )))
        }
    }
}

impl From<Attribute> for u8 {
    fn from(val: Attribute) -> Self {
        match val {
            Attribute::Origin(_, _) => Attribute::ORIGIN,
            Attribute::ASPath(_, _) => Attribute::AS_PATH,
            Attribute::NextHop(_, _) => Attribute::NEXT_HOP,
            Attribute::MultiExitDisc(_, _) => Attribute::MULTI_EXIT_DISC,
            Attribute::LocalPref(_, _) => Attribute::LOCAL_PREF,
            Attribute::AtomicAggregate(_) => Attribute::ATOMIC_AGGREGATE,
            Attribute::Aggregator(_, _, _) => Attribute::AGGREGATOR,
            Attribute::Communities(_, _) => Attribute::COMMUNITIES,
            Attribute::ExtendedCommunities(_, _, _) => Attribute::EXTENDED_COMMUNITIES,
            Attribute::MPReachNLRI(_, _, _, _) => Attribute::MP_REACH_NLRI,
            Attribute::MPUnReachNLRI(_, _, _) => Attribute::MP_UNREACH_NLRI,
            Attribute::AS4Path(_, _) => Attribute::AS4_PATH,
            Attribute::AS4Aggregator(_, _, _) => Attribute::AS4_AGGREGATOR,
            Attribute::Unsupported(b, _) => b.code,
        }
    }
}

impl TryFrom<prost_types::Any> for Attribute {
    type Error = Error;
    fn try_from(value: prost_types::Any) -> Result<Self, Self::Error> {
        if value.type_url == sartd_util::type_url("OriginAttribute") {
            let a = sartd_proto::sart::OriginAttribute::decode(&*value.value)
                .map_err(|e| Error::Config(ConfigError::InvalidArgument(e.to_string())))?;
            Ok(Attribute::new_origin(a.value as u8)?)
        } else if value.type_url == sartd_util::type_url("AsPathAttribute") {
            let a = sartd_proto::sart::AsPathAttribute::decode(&*value.value)
                .map_err(|e| Error::Config(ConfigError::InvalidArgument(e.to_string())))?;
            let seq = match a
                .segments
                .iter()
                .find(|s| s.r#type == Attribute::AS_SEQUENCE as i32)
            {
                Some(seq) => seq.elm.clone(),
                None => Vec::new(),
            };
            let set = match a
                .segments
                .iter()
                .find(|s| s.r#type == Attribute::AS_SET as i32)
            {
                Some(set) => set.elm.clone(),
                None => Vec::new(),
            };
            Ok(Attribute::new_as_path(seq, set, true)?)
        } else if value.type_url == sartd_util::type_url("NextHopAttribute") {
            let a = sartd_proto::sart::NextHopAttribute::decode(&*value.value)
                .map_err(|e| Error::Config(ConfigError::InvalidArgument(e.to_string())))?;
            Ok(Attribute::new_nexthop(
                a.value
                    .parse()
                    .map_err(|_| Error::Config(ConfigError::InvalidData))?,
            ))
        } else if value.type_url == sartd_util::type_url("LocalPrefAttribute") {
            let a = sartd_proto::sart::LocalPrefAttribute::decode(&*value.value)
                .map_err(|e| Error::Config(ConfigError::InvalidArgument(e.to_string())))?;
            Ok(Attribute::new_local_pref(a.value)?)
        } else if value.type_url == sartd_util::type_url("MultiExitDiscAttribute") {
            let a = sartd_proto::sart::MultiExitDiscAttribute::decode(&*value.value)
                .map_err(|e| Error::Config(ConfigError::InvalidArgument(e.to_string())))?;
            Ok(Attribute::new_med(a.value)?)
        } else if value.type_url == sartd_util::type_url("AtomicAggregateAttribute") {
            Ok(Attribute::AtomicAggregate(Base::new(
                0,
                Attribute::ATOMIC_AGGREGATE,
            )))
        } else if value.type_url == sartd_util::type_url("AggregatorAttribute") {
            let a = sartd_proto::sart::AggregatorAttribute::decode(&*value.value)
                .map_err(|e| Error::Config(ConfigError::InvalidArgument(e.to_string())))?;
            Ok(Attribute::AS4Aggregator(
                Base::new(
                    Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL,
                    Attribute::AGGREGATOR,
                ),
                a.asn,
                a.address
                    .parse()
                    .map_err(|_| Error::Config(ConfigError::InvalidData))?,
            ))
        } else {
            Err(Error::Config(ConfigError::InvalidArgument(
                "invalid attribute".to_string(),
            )))
        }
    }
}

impl From<&Attribute> for prost_types::Any {
    fn from(attr: &Attribute) -> Self {
        match attr {
            Attribute::Origin(_, val) => sartd_util::to_any(
                sartd_proto::sart::OriginAttribute { value: *val as u32 },
                "OriginAttribute",
            ),
            Attribute::ASPath(_, segments) => sartd_util::to_any(
                sartd_proto::sart::AsPathAttribute {
                    segments: segments
                        .iter()
                        .map(sartd_proto::sart::AsSegment::from)
                        .collect(),
                },
                "AsPathAttribute",
            ),
            Attribute::NextHop(_, addr) => sartd_util::to_any(
                sartd_proto::sart::NextHopAttribute {
                    value: addr.to_string(),
                },
                "NextHopAttribute",
            ),
            Attribute::LocalPref(_, val) => sartd_util::to_any(
                sartd_proto::sart::LocalPrefAttribute { value: *val },
                "LocalPrefAttribute",
            ),
            Attribute::MultiExitDisc(_, val) => sartd_util::to_any(
                sartd_proto::sart::MultiExitDiscAttribute { value: *val },
                "MultiExitDiscAttribute",
            ),
            Attribute::AtomicAggregate(_) => sartd_util::to_any(
                sartd_proto::sart::AtomicAggregateAttribute {},
                "AtomicAggregateAttribute",
            ),
            Attribute::Aggregator(_, asn, addr) => sartd_util::to_any(
                sartd_proto::sart::AggregatorAttribute {
                    asn: *asn,
                    address: addr.to_string(),
                },
                "AggregatorAttribute",
            ),
            Attribute::Unsupported(b, data) => sartd_util::to_any(
                sartd_proto::sart::UnknownAttribute {
                    flags: b.flag as u32,
                    code: b.code as u32,
                    data: data.to_vec(),
                },
                "UnknownAttribute",
            ),
            Attribute::Communities(_, _) => todo!(),
            Attribute::MPReachNLRI(_, _, _, _) => todo!(),
            Attribute::MPUnReachNLRI(_, _, _) => todo!(),
            Attribute::ExtendedCommunities(_, _, _) => todo!(),
            Attribute::AS4Path(_, _) => todo!(),
            Attribute::AS4Aggregator(_, _, _) => todo!(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Base {
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

    pub fn is_transitive(&self) -> bool {
        (self.flag & Attribute::FLAG_TRANSITIVE) != 0
    }

    pub fn is_optional(&self) -> bool {
        (self.flag & Attribute::FLAG_OPTIONAL) != 0
    }

    fn is_partial(&self) -> bool {
        (self.flag & Attribute::FLAG_PARTIAL) != 0
    }

    pub fn set_partial(&mut self) {
        self.flag += Attribute::FLAG_PARTIAL;
    }

    fn validate_attribute_flag(&self) -> bool {
        match self.code {
            // ORIGIN is a well-known mandatory attribute.
            Attribute::ORIGIN => !self.is_optional(),
            // AS_PATH is a well-known mandatory attribute.
            Attribute::AS_PATH => !self.is_optional(),
            // NEXT_HOP is a well-known mandatory attribute.
            Attribute::NEXT_HOP => !self.is_optional(),
            // MULTI_EXIT_DISC is an optional non-transitive attribute
            Attribute::MULTI_EXIT_DISC => self.is_optional() && !self.is_transitive(),
            // LOCAL_PREF is a well-known attribute
            Attribute::LOCAL_PREF => true,
            // ATOMIC_AGGREGATE is a well-known discretionary attribute.
            Attribute::ATOMIC_AGGREGATE => true,
            // AGGREGATOR is an optional transitive attribute,
            Attribute::AGGREGATOR => self.is_optional(),
            Attribute::COMMUNITIES => self.is_optional(),
            Attribute::MP_REACH_NLRI => self.is_optional(),
            Attribute::MP_UNREACH_NLRI => self.is_optional(),
            Attribute::EXTENDED_COMMUNITIES => self.is_optional(),
            Attribute::AS4_PATH => self.is_optional(),
            Attribute::AS4_AGGREGATOR => self.is_optional(),
            _ => false,
        }
    }
}

impl<'a> From<&'a Base> for u16 {
    fn from(val: &'a Base) -> Self {
        ((val.flag as u16) << 8) + (val.code as u16)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ASSegment {
    pub segment_type: u8,
    pub segments: Vec<u32>,
}

impl ASSegment {
    fn new(segment_type: u8, segments: Vec<u32>) -> Self {
        Self {
            segment_type,
            segments,
        }
    }

    fn len(&self) -> usize {
        self.segments.len()
    }
}

impl TryFrom<prost_types::Any> for ASSegment {
    type Error = Error;
    fn try_from(value: prost_types::Any) -> Result<Self, Self::Error> {
        if value.type_url == sartd_util::type_url("AsSegment") {
            let s = sartd_proto::sart::AsSegment::decode(&*value.value)
                .map_err(|_| Error::Config(ConfigError::InvalidData))?;
            Ok(ASSegment {
                segment_type: s.r#type as u8,
                segments: s.elm,
            })
        } else {
            Err(Error::Config(ConfigError::InvalidArgument(
                "failed to parse AS Segement".to_string(),
            )))
        }
    }
}

impl From<&ASSegment> for sartd_proto::sart::AsSegment {
    fn from(segment: &ASSegment) -> Self {
        sartd_proto::sart::AsSegment {
            r#type: segment.segment_type as i32,
            elm: segment.segments.clone(),
        }
    }
}

impl From<&ASSegment> for prost_types::Any {
    fn from(segment: &ASSegment) -> Self {
        sartd_util::to_any(
            sartd_proto::sart::AsSegment {
                r#type: segment.segment_type as i32,
                elm: segment.segments.clone(),
            },
            "AsSegment",
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::family::{AddressFamily, Afi, Safi};
    use crate::packet::attribute::{ASSegment, Attribute, Base};
    use crate::packet::prefix::Prefix;
    use bytes::BytesMut;
    use rstest::rstest;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use std::str::FromStr;

    #[rstest(
		input,
        as4_enabled,
        add_path_enabled,
		expected,
		case(vec![0x40, 0x01, 0x01, 0x00], false, false, Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_IGP)),
		case(vec![0x40, 0x01, 0x01, 0x01], false, false, Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_EGP)),
		case(vec![0x40, 0x01, 0x01, 0x02], false, false, Attribute::Origin(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::ORIGIN}, Attribute::ORIGIN_INCOMPLETE)),
	)]
    fn works_attribute_decode_origin(
        input: Vec<u8>,
        as4_enabled: bool,
        add_path_enabled: bool,
        expected: Attribute,
    ) {
        let mut buf = BytesMut::from(input.as_slice());
        match Attribute::decode(&mut buf, as4_enabled, add_path_enabled) {
            Ok(origin) => assert_eq!(expected, origin),
            Err(_) => panic!("failed"),
        }
    }

    #[rstest(
		input,
        as4_enabled,
        _add_path_enabled,
		expected,
		case(vec![0x40, 0x02, 0x04, 0x02, 0x01, 0xfd, 0xe9], false, false, Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![65001]}])),
		case(vec![0x40, 0x02, 0x06, 0x02, 0x02, 0x5b, 0xa0, 0x5b, 0xa0], false, false, Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![23456, 23456]}])),
		case(vec![0x40, 0x02, 0x0a, 0x02, 0x01, 0x00, 0x1e, 0x01, 0x02, 0x00, 0x0a, 0x00, 0x14], false, false, Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH},
			vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![30]}, ASSegment{segment_type: Attribute::AS_SET, segments: vec![10 ,20]}],
		)),
        case(vec![0x40, 0x02, 0x06, 0x02, 0x01, 0xfd, 0xe8, 0xfd, 0xe8], true, false, Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![4259905000]}])),
        case(vec![0x40, 0x02, 0x0e, 0x02, 0x03, 0x00, 0x0a, 0x00,  0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03], true, false, Attribute::ASPath(Base{flag: Attribute::FLAG_TRANSITIVE, code: Attribute::AS_PATH}, vec![ASSegment{segment_type: Attribute::AS_SEQUENCE, segments: vec![655361, 2, 3]}])),
	)]
    fn works_attribute_decode_as_path(
        input: Vec<u8>,
        as4_enabled: bool,
        _add_path_enabled: bool,
        expected: Attribute,
    ) {
        let mut buf = BytesMut::from(input.as_slice());
        match Attribute::decode(&mut buf, as4_enabled, false) {
            Ok(as_path) => assert_eq!(expected, as_path),
            Err(_) => panic!("failed"),
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
            Err(_) => panic!("failed"),
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
            Err(_) => panic!("failed"),
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
    fn works_attribute_encode(
        attr: Attribute,
        as4_enabled: bool,
        add_path_enabled: bool,
        expected: Vec<u8>,
    ) {
        let mut buf = BytesMut::new();
        attr.encode(&mut buf, as4_enabled, add_path_enabled)
            .unwrap();
        assert_eq!(expected, buf.to_vec())
    }
}
