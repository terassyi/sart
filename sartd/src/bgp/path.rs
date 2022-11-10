use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use ipnet::IpNet;

use crate::bgp::packet::attribute::Attribute;
use crate::bgp::packet::attribute::ASSegment;
use crate::bgp::packet::prefix::Prefix;
use crate::bgp::error::Error;

use super::family::AddressFamily;
use super::packet::message::Message;

#[derive(Debug, PartialEq)]
pub(crate) struct Path {
	id: u64,
	local_id: Ipv4Addr,
	local_asn: u32,
	peer_id: Ipv4Addr,
	peer_addr: IpAddr,
	peer_asn: u32,
	family: AddressFamily,
	origin: u8,
	local_pref: u32,
	med: u32,
	as_sequence: Vec<u32>,
	as_set: Vec<u32>,
	next_hops: Vec<IpAddr>,
	propagate_attributes: Vec<Attribute>,
	prefix: IpNet,
}

impl Path {
}

#[derive(Debug, Hash)]
pub(crate) struct PathBuilder {
	local_id: Ipv4Addr,
	local_asn: u32,
	peer_id: Ipv4Addr,
	peer_addr: IpAddr,
	peer_asn: u32,
	family: AddressFamily,
	next_hop: Vec<IpAddr>,
	origin: u8,
	as_sequence: Vec<u32>,
	as_set: Vec<u32>,
	as4_sequence: Vec<u32>,
	as4_set: Vec<u32>,
	local_pref: u32,
	med: u32,
	propagate_attrs: Vec<Attribute>,
	nlri: Vec<Prefix>,
}

impl PathBuilder {
	pub fn builder(local_id: Ipv4Addr, local_asn: u32, peer_id: Ipv4Addr, peer_addr: IpAddr, peer_asn: u32) -> Self {
		Self {
			local_id,
			local_asn,
			peer_id,
			peer_addr,
			peer_asn,
			family: AddressFamily::ipv4_unicast(),
			next_hop: Vec::new(),
			origin: Attribute::ORIGIN_IGP,
			as_sequence: Vec::new(),
			as_set: Vec::new(),
			as4_sequence: Vec::new(),
			as4_set: Vec::new(),
			local_pref: 0,
			med: 0,
			propagate_attrs: Vec::new(),
			nlri: Vec::new(),
		}
	}

	pub fn builder_local(network: &str, local_id: Ipv4Addr, local_asn: u32) -> Self {
		Self {
			local_id,
			local_asn,
			peer_id: Ipv4Addr::new(0, 0, 0, 0),
			peer_addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
			peer_asn: 0,
			family: AddressFamily::ipv4_unicast(),
			next_hop: Vec::new(),
			origin: Attribute::ORIGIN_IGP,
			as_sequence: Vec::new(),
			as_set: Vec::new(),
			as4_sequence: Vec::new(),
			as4_set: Vec::new(),
			local_pref: 0,
			med: 0,
			propagate_attrs: Vec::new(),
			nlri: vec![Prefix::from(IpNet::from_str(network).unwrap())],
		}
	}

	pub fn next_hop(&mut self, next_hop: IpAddr) -> &mut Self {
		self.next_hop.push(next_hop);
		self
	}

	pub fn origin(&mut self, val: u8) -> &mut Self {
		self.origin = val;
		self
	}

	pub fn as_segment(&mut self, mut segment: ASSegment, as4: bool) -> &mut Self {
		if as4 {
			if segment.segment_type == Attribute::AS_SEQUENCE {
				self.as4_sequence.append(&mut segment.segments);
			} else {
				self.as4_set.append(&mut segment.segments);
			}
		} else {
			if segment.segment_type == Attribute::AS_SEQUENCE {
				self.as_sequence.append(&mut segment.segments);
			} else {
				self.as_set.append(&mut segment.segments);
			}
		}
		self
	}

	pub fn as_segments(&mut self, segments: Vec<ASSegment>, as4: bool) -> &mut Self {
		if as4 {
			for mut segment in segments.into_iter() {
				if segment.segment_type == Attribute::AS_SEQUENCE {
					self.as4_sequence.append(&mut segment.segments);
				} else {
					self.as4_set.append(&mut segment.segments);
				}
			}
		} else {
			for mut segment in segments.into_iter() {
				if segment.segment_type == Attribute::AS_SEQUENCE {
					self.as_sequence.append(&mut segment.segments);
				} else {
					self.as_set.append(&mut segment.segments);
				}
			}
		}
		self
	}

	pub fn as_sequence(&mut self, mut sequence: Vec<u32>, as4: bool) -> &mut Self {
		if as4 {
			self.as4_sequence.append(&mut sequence);
		} else {
			self.as_sequence.append(&mut sequence);
		}
		self
	}

	pub fn as_set(&mut self, mut set: Vec<u32>, as4: bool) -> &mut Self {
		if as4 {
			self.as4_sequence.append(&mut set);
		} else {
			self.as_sequence.append(&mut set);
		}
		self
	}

	pub fn local_pref(&mut self, val: u32) -> &mut Self {
		self.local_pref = val;
		self
	}

	pub fn med(&mut self, val: u32) -> &mut Self {
		self.med = val;
		self
	}

	pub fn propagate_attr(&mut self, attr: Attribute) -> &mut Self {
		self.propagate_attrs.push(attr);
		self
	}

	pub fn propagate_attrs(&mut self, mut attrs: Vec<Attribute>) -> &mut Self {
		self.propagate_attrs.append(&mut attrs);
		self
	}

	fn family(&mut self, family: AddressFamily) -> &mut Self {
		self.family = family;
		self
	}

	pub fn attr(&mut self, attr: Attribute) -> Result<&mut Self, Error> {
		if attr.is_optional() && attr.is_transitive() && attr.is_recognized() {
			self.propagate_attr(attr.clone());
		}
		match attr {
            Attribute::Origin(_, val) => Ok(self.origin(val)),
            Attribute::ASPath(_, segments) => Ok(self.as_segments(segments, false)),
            Attribute::NextHop(_, val) => Ok(self.next_hop(IpAddr::V4(val))),
            Attribute::MultiExitDisc(_, val) => Ok(self.med(val)),
            Attribute::LocalPref(_, val) => Ok(self.local_pref(val)),
            Attribute::AtomicAggregate(b) => Ok(self),
            Attribute::Aggregator(b, _, _) => Ok(self),
            Attribute::Communities(b, _) => Ok(self),
            Attribute::ExtendedCommunities(b, _, _) => Ok(self),
            Attribute::MPReachNLRI(_, family, mut next_hops, nlri) => {
				self.next_hop.append(&mut next_hops);
				Ok(self
					.nlri(nlri)
					.family(family))
			},
            Attribute::MPUnReachNLRI(b, _, _) => Ok(self),
            Attribute::AS4Path(_, segments) => Ok(self.as_segments(segments, true)),
            Attribute::AS4Aggregator(b, _, _) => Ok(self),
            Attribute::Unsupported(mut b, data) => {
				if b.is_optional() && b.is_transitive() {
					b.set_partial();
				}
				Ok(self.propagate_attr(Attribute::Unsupported(b, data)))
			},
		}
	}

	pub fn nlri(&mut self, mut nlri: Vec<Prefix>) -> &mut Self {
		self.nlri.append(&mut nlri);
		self
	}

	pub fn build(&self) -> Result<Vec<Path>, Error> {
		let mut i = 0;
		let seq: Vec<u32> = self.as_sequence.iter().map(|&s| {
			if s == Message::AS_TRANS {
				let ss = self.as4_sequence[i];
				i += 1;
				ss
			} else {
				s
			}
		}).collect();
		let set: Vec<u32> = self.as_set.iter().map(|&s| {
			if s == Message::AS_TRANS {
				let ss = self.as4_sequence[i];
				i += 1;
				ss
			} else {
				s
			}
		}).collect();
		let mut h = DefaultHasher::new();
		self.hash(&mut h);
		Ok(self.nlri.iter().map(|p| {
			Path {
				id: h.finish(),
    			local_id: self.local_id,
    			local_asn: self.local_asn,
    			peer_id: self.peer_id,
    			peer_addr: self.peer_addr,
    			peer_asn: self.peer_asn,
    			family: self.family,
    			origin: self.origin,
				local_pref: self.local_pref,
				med: self.med,
    			as_sequence: seq.clone(),
    			as_set: set.clone(),
    			next_hops: self.next_hop.clone(),
				propagate_attributes: self.propagate_attrs.clone(),
    			prefix: p.into(),
			}
		}).collect())
	}
}

#[cfg(test)]
mod tests {
	use rstest::rstest;
	use std::net::{IpAddr, Ipv4Addr};
	use ipnet::{IpNet, Ipv4Net};
	use crate::bgp::packet::message::Message;
	use crate::bgp::family::AddressFamily;
	use crate::bgp::packet::attribute::{Attribute, Base, ASSegment};
	use crate::bgp::packet::prefix::Prefix;
	use crate::bgp::path::Path;

use super::PathBuilder;

	#[rstest(
		msg,
		expected,
		case(Message::Update {
            	withdrawn_routes: Vec::new(),
            	attributes: vec![
              		Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_INCOMPLETE),
              		Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![30]}, ASSegment{ segment_type: Attribute::AS_SET, segments: vec![10, 20]}]),
              		Attribute::NextHop(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::NEXT_HOP), Ipv4Addr::new(10, 0, 0, 9)),
              		Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
              		Attribute::Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE, Attribute::AGGREGATOR), 30, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))),
            	],
            	nlri: vec![Prefix::new(IpNet::V4(Ipv4Net::new(Ipv4Addr::new(172, 16, 0, 0), 21).unwrap()), None)],
        	},
			vec![
				Path{
					id: 0,
    				local_id: Ipv4Addr::new(1, 1, 1, 1),
    				local_asn: 100,
    				peer_id: Ipv4Addr::new(2, 2, 2, 2),
    				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
    				peer_asn: 200,
    				family: AddressFamily::ipv4_unicast(),
    				origin: Attribute::ORIGIN_INCOMPLETE,
					local_pref: 0,
					med: 0,
    				as_sequence: vec![30],
    				as_set: vec![10, 20],
    				next_hops: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))],
					propagate_attributes: vec![
						Attribute::Aggregator(Base::new(Attribute::FLAG_OPTIONAL + Attribute::FLAG_TRANSITIVE, Attribute::AGGREGATOR), 30, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))),
					],
    				prefix: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(172, 16, 0, 0), 21).unwrap()),
				}
			]
		),
		case(
			Message::Update {
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
        	},
			vec![
				Path{
					id: 0,
    				local_id: Ipv4Addr::new(1, 1, 1, 1),
    				local_asn: 100,
    				peer_id: Ipv4Addr::new(2, 2, 2, 2),
    				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
    				peer_asn: 200,
    				family: AddressFamily::ipv4_unicast(),
    				origin: Attribute::ORIGIN_IGP,
					local_pref: 0,
					med: 0,
    				as_sequence: vec![655361, 2621441],
    				as_set: vec![],
    				next_hops: vec![IpAddr::V4(Ipv4Addr::new(172, 16, 3, 1))],
					propagate_attributes: vec![
            	    	Attribute::AS4Path(Base::new(Attribute::FLAG_TRANSITIVE + Attribute::FLAG_OPTIONAL, Attribute::AS4_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![655361, 2621441]}]),
					],
    				prefix: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(40, 0, 0, 0), 8).unwrap()),
				}
			],
		),
		case(
			Message::Update {
            	withdrawn_routes: Vec::new(),
            	attributes: vec![
            	    Attribute::Origin(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::ORIGIN), Attribute::ORIGIN_IGP),
            	    Attribute::ASPath(Base::new(Attribute::FLAG_TRANSITIVE, Attribute::AS_PATH), vec![ASSegment{ segment_type: Attribute::AS_SEQUENCE, segments: vec![65001]}]),
            	    Attribute::MultiExitDisc(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MULTI_EXIT_DISC), 0),
            	    Attribute::MPReachNLRI(Base::new(Attribute::FLAG_OPTIONAL, Attribute::MP_REACH_NLRI), AddressFamily::ipv6_unicast(), vec![IpAddr::V6("2001:db8::1".parse().unwrap()), IpAddr::V6("fe80::c001:bff:fe7e:0".parse().unwrap())], vec![Prefix::new("2001:db8:1:2::/64".parse().unwrap(), None), Prefix::new("2001:db8:1:1::/64".parse().unwrap(), None), Prefix::new("2001:db8:1::/64".parse().unwrap(), None)]),
            	],
            	nlri: Vec::new(),
        	},
			vec![
				Path{
					id: 0,
    				local_id: Ipv4Addr::new(1, 1, 1, 1),
    				local_asn: 100,
    				peer_id: Ipv4Addr::new(2, 2, 2, 2),
    				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
    				peer_asn: 200,
    				family: AddressFamily::ipv6_unicast(),
    				origin: Attribute::ORIGIN_IGP,
					local_pref: 0,
					med: 0,
    				as_sequence: vec![65001],
    				as_set: vec![],
    				next_hops: vec![IpAddr::V6("2001:db8::1".parse().unwrap()), IpAddr::V6("fe80::c001:bff:fe7e:0".parse().unwrap())],
					propagate_attributes: Vec::new(),
    				prefix: IpNet::V6("2001:db8:1:2::/64".parse().unwrap()),
				},
				Path{
					id: 0,
    				local_id: Ipv4Addr::new(1, 1, 1, 1),
    				local_asn: 100,
    				peer_id: Ipv4Addr::new(2, 2, 2, 2),
    				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
    				peer_asn: 200,
    				family: AddressFamily::ipv6_unicast(),
    				origin: Attribute::ORIGIN_IGP,
					local_pref: 0,
					med: 0,
    				as_sequence: vec![65001],
    				as_set: vec![],
    				next_hops: vec![IpAddr::V6("2001:db8::1".parse().unwrap()), IpAddr::V6("fe80::c001:bff:fe7e:0".parse().unwrap())],
					propagate_attributes: Vec::new(),
    				prefix: IpNet::V6("2001:db8:1:1::/64".parse().unwrap()),
				},
				Path{
					id: 0,
    				local_id: Ipv4Addr::new(1, 1, 1, 1),
    				local_asn: 100,
    				peer_id: Ipv4Addr::new(2, 2, 2, 2),
    				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
    				peer_asn: 200,
    				family: AddressFamily::ipv6_unicast(),
    				origin: Attribute::ORIGIN_IGP,
					local_pref: 0,
					med: 0,
    				as_sequence: vec![65001],
    				as_set: vec![],
    				next_hops: vec![IpAddr::V6("2001:db8::1".parse().unwrap()), IpAddr::V6("fe80::c001:bff:fe7e:0".parse().unwrap())],
					propagate_attributes: Vec::new(),
    				prefix: IpNet::V6("2001:db8:1::/64".parse().unwrap()),
				},
			],
		),
		case(
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
			vec![
				Path{
					id: 0,
    				local_id: Ipv4Addr::new(1, 1, 1, 1),
    				local_asn: 100,
    				peer_id: Ipv4Addr::new(2, 2, 2, 2),
    				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
    				peer_asn: 200,
    				family: AddressFamily::ipv4_unicast(),
    				origin: Attribute::ORIGIN_IGP,
					local_pref: 0,
					med: 0,
    				as_sequence: vec![65100],
    				as_set: vec![],
    				next_hops: vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))],
					propagate_attributes: Vec::new(),
    				prefix: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 3, 0), 24).unwrap()),
				},
				Path{
					id: 0,
    				local_id: Ipv4Addr::new(1, 1, 1, 1),
    				local_asn: 100,
    				peer_id: Ipv4Addr::new(2, 2, 2, 2),
    				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
    				peer_asn: 200,
    				family: AddressFamily::ipv4_unicast(),
    				origin: Attribute::ORIGIN_IGP,
					local_pref: 0,
					med: 0,
    				as_sequence: vec![65100],
    				as_set: vec![],
    				next_hops: vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))],
					propagate_attributes: Vec::new(),
    				prefix: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 2, 0), 24).unwrap()),
				},
				Path{
					id: 0,
    				local_id: Ipv4Addr::new(1, 1, 1, 1),
    				local_asn: 100,
    				peer_id: Ipv4Addr::new(2, 2, 2, 2),
    				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
    				peer_asn: 200,
    				family: AddressFamily::ipv4_unicast(),
    				origin: Attribute::ORIGIN_IGP,
					local_pref: 0,
					med: 0,
    				as_sequence: vec![65100],
    				as_set: vec![],
    				next_hops: vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))],
					propagate_attributes: Vec::new(),
    				prefix: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 1, 0), 24).unwrap()),
				},
			]
		),
	)]
	fn works_build_path(msg: Message, expected: Vec<Path>) {
		let (_withdrawn_routes, attributes, nlri) = msg.to_update().unwrap();
		let mut builder = PathBuilder::builder(Ipv4Addr::new(1, 1, 1, 1), 100, Ipv4Addr::new(2, 2, 2, 2), IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)), 200);
		for attr in attributes.into_iter() {
			builder.attr(attr).unwrap();
		}
		let paths: Vec<Path> = builder.nlri(nlri)
				.build().unwrap()
				.into_iter()
				.map(|mut path| {
					path.id = 0;
					path
				}).collect();
		assert_eq!(expected, paths);
	}
}
