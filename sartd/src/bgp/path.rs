use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use ipnet::IpNet;
use tokio::time::Instant;

use crate::bgp::error::Error;
use crate::bgp::packet::attribute::ASSegment;
use crate::bgp::packet::attribute::Attribute;
use crate::bgp::packet::prefix::Prefix;

use super::error::UpdateMessageError;
use super::family::AddressFamily;

#[derive(Debug, Clone)]
pub(crate) struct Path {
    pub id: u64,
    pub best: bool,
    pub reason: BestPathReason,
    pub timestamp: Instant,
    pub local_id: Ipv4Addr,
    pub local_asn: u32,
    pub peer_id: Ipv4Addr,
    pub peer_addr: IpAddr,
    pub peer_asn: u32,
    pub family: AddressFamily,
    pub origin: u8,
    pub local_pref: u32,
    pub med: u32,
    pub as_sequence: Vec<u32>,
    pub as_set: Vec<u32>,
    pub next_hops: Vec<IpAddr>,
    pub propagate_attributes: Vec<Attribute>,
    pub prefix: IpNet,
}

impl std::cmp::PartialEq for Path {
    fn eq(&self, other: &Self) -> bool {
        self.local_id.eq(&other.local_id)
            || self.local_asn == other.local_asn
            || self.peer_id.eq(&other.peer_id)
            || self.peer_addr.eq(&other.peer_addr)
            || self.peer_asn == other.peer_asn
            || self.family.eq(&other.family)
            || self.origin == other.origin
            || self.local_pref == other.local_pref
            || self.med == other.med
            || self.as_sequence.eq(&other.as_sequence)
            || self.as_set.eq(&other.as_set)
            || self.next_hops.eq(&other.next_hops)
            || self.propagate_attributes.eq(&other.propagate_attributes)
            || self.prefix.eq(&other.prefix)
    }
}

impl Path {
    pub fn prefix(&self) -> IpNet {
        self.prefix
    }

    pub fn family(&self) -> AddressFamily {
        self.family
    }

    pub fn is_local_originated(&self) -> bool {
        self.local_id == self.peer_id
    }

    pub fn is_external(&self) -> bool {
        !self.is_local_originated() && self.local_asn != self.peer_asn
    }

    pub fn kind(&self) -> PathKind {
        if self.local_id.eq(&self.peer_id) {
            return PathKind::Local;
        }
        if self.local_asn == self.peer_asn {
            return PathKind::Internal;
        }
        PathKind::External
    }

    pub fn has_as(&self, asn: u32) -> bool {
        if self.as_sequence.contains(&asn) {
            return true;
        }
        if self.as_set.contains(&asn) {
            return true;
        }
        false
    }

    pub fn has_own_as(&self) -> bool {
        self.has_as(self.local_asn)
    }

    pub fn is_equal_cost(&self, path: &Path) -> bool {
        if self.local_pref != path.local_pref {
            return false;
        }
        if self.as_sequence.len() != path.as_sequence.len() {
            return false;
        }
        if self.origin != path.origin {
            return false;
        }
        if self.med != path.med {
            return false;
        }
        if (self.local_asn == self.peer_asn) != (path.local_asn == path.peer_asn) {
            return false;
        }
        true
    }

    pub fn to_outgoing(
        &mut self,
        is_ibgp: bool,
        asn: u32,
        next_hops: Vec<IpAddr>,
    ) -> Result<&mut Self, Error> {
        if self.kind() == PathKind::Internal {
            if !is_ibgp {
                self.origin = Attribute::ORIGIN_INCOMPLETE;
            }
        }

        // add as_path
        if !is_ibgp {
            self.as_sequence.insert(0, asn);
        }

        // replace next hop addresses
        if is_ibgp && self.kind() == PathKind::External {
            // ibgp peer
            return Ok(self);
        }
        self.next_hops = next_hops;
        // local_pref
        if is_ibgp && self.local_pref == 0 {
            self.local_pref = 100;
        }
        Ok(self)
    }

    pub fn attributes(&self, as4_enabled: bool) -> Result<Vec<Attribute>, Error> {
        let mut attrs = Vec::new();

        // origin
        attrs.push(Attribute::new_origin(self.origin)?);
        // as_path
        if as4_enabled {
            attrs.push(Attribute::new_as_path(
                self.as_sequence.clone(),
                self.as_set.clone(),
                true,
            )?)
        } else {
            let as4_seq = self
                .as_sequence
                .iter()
                .filter_map(|&n| {
                    if n > Attribute::AS_2BYTES_LIMIT {
                        Some(n)
                    } else {
                        None
                    }
                })
                .collect::<Vec<u32>>();
            let as4_set = self
                .as_set
                .iter()
                .filter_map(|&n| {
                    if n > Attribute::AS_2BYTES_LIMIT {
                        Some(n)
                    } else {
                        None
                    }
                })
                .collect::<Vec<u32>>();
            let as_seq = self
                .as_sequence
                .iter()
                .map(|&n| {
                    if n > Attribute::AS_2BYTES_LIMIT {
                        Attribute::AS_TRANS
                    } else {
                        n
                    }
                })
                .collect::<Vec<u32>>();
            let mut as_set = self
                .as_set
                .iter()
                .filter_map(|&n| {
                    if n <= Attribute::AS_2BYTES_LIMIT {
                        Some(n)
                    } else {
                        None
                    }
                })
                .collect::<Vec<u32>>();
            if as_set.len() < self.as_set.len() {
                as_set.push(Attribute::AS_TRANS);
            }
            attrs.push(Attribute::new_as_path(as_seq, as_set, true)?);
            if !as4_seq.is_empty() || !as4_set.is_empty() {
                attrs.push(Attribute::new_as4_path(as4_seq, as4_set, true)?);
            }
        }
        // next hop
        if self.next_hops.is_empty() {
            return Err(Error::UpdateMessage(
                UpdateMessageError::InvalidNextHopAttribute,
            ));
        }
        match self.next_hops[0] {
            IpAddr::V4(addr) => attrs.push(Attribute::new_nexthop(addr)),
            IpAddr::V6(_) => {} // mp_reach_nlri will be handled later
        }

        if self.local_pref != 0 {
            attrs.push(Attribute::new_local_pref(self.local_pref)?);
        }
        if self.med != 0 {
            attrs.push(Attribute::new_med(self.med)?);
        }

        Ok(attrs)
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone, Copy)]
pub enum PathKind {
    Local,
    Internal,
    External,
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
    pub fn builder(
        local_id: Ipv4Addr,
        local_asn: u32,
        peer_id: Ipv4Addr,
        peer_addr: IpAddr,
        peer_asn: u32,
    ) -> Self {
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

    pub fn builder_local(local_id: Ipv4Addr, local_asn: u32) -> Self {
        Self {
            local_id,
            local_asn,
            peer_id: local_id,
            peer_addr: IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)),
            peer_asn: local_asn,
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
            nlri: vec![],
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
            Attribute::AtomicAggregate(_) => Ok(self),
            Attribute::Aggregator(_, _, _) => Ok(self),
            Attribute::Communities(_, _) => Ok(self),
            Attribute::ExtendedCommunities(_, _, _) => Ok(self),
            Attribute::MPReachNLRI(_, family, mut next_hops, nlri) => {
                self.next_hop.append(&mut next_hops);
                Ok(self.nlri(nlri).family(family))
            }
            Attribute::MPUnReachNLRI(_, _, _) => Ok(self),
            Attribute::AS4Path(_, segments) => Ok(self.as_segments(segments, true)),
            Attribute::AS4Aggregator(_, _, _) => Ok(self),
            Attribute::Unsupported(mut b, data) => {
                if b.is_optional() && b.is_transitive() {
                    b.set_partial();
                }
                Ok(self.propagate_attr(Attribute::Unsupported(b, data)))
            }
        }
    }

    pub fn nlri(&mut self, mut nlri: Vec<Prefix>) -> &mut Self {
        self.nlri.append(&mut nlri);
        self
    }

    pub fn build(&self) -> Result<Vec<Path>, Error> {
        let mut i = 0;
        let seq: Vec<u32> = self
            .as_sequence
            .iter()
            .map(|&s| {
                if s == Attribute::AS_TRANS {
                    let ss = self.as4_sequence[i];
                    i += 1;
                    ss
                } else {
                    s
                }
            })
            .collect();
        let set: Vec<u32> = self
            .as_set
            .iter()
            .map(|&s| {
                if s == Attribute::AS_TRANS {
                    let ss = self.as4_sequence[i];
                    i += 1;
                    ss
                } else {
                    s
                }
            })
            .collect();
        let mut h = DefaultHasher::new();
        self.hash(&mut h);
        Ok(self
            .nlri
            .iter()
            .map(|p| Path {
                id: h.finish(),
                best: false,
                reason: BestPathReason::NotBest,
                timestamp: Instant::now(),
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
            })
            .collect())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BestPathReason {
    OnlyPath = 0,
    Weight = 1,
    LocalPref = 2,
    LocalOriginated = 3,
    ASPath = 4,
    Origin = 5,
    MultiExitDisc = 6,
    External = 7,
    IGPMetric = 8,
    OlderRoute = 9,
    RouterId = 10,
    PeerAddr = 11,
    EqualCostMiltiPath = 99,
    NotBest = 100,
}

#[cfg(test)]
mod tests {
    use crate::bgp::family::AddressFamily;
    use crate::bgp::packet::attribute::{ASSegment, Attribute, Base};
    use crate::bgp::packet::message::Message;
    use crate::bgp::packet::prefix::Prefix;
    use crate::bgp::path::BestPathReason;
    use crate::bgp::path::Path;
    use ipnet::{IpNet, Ipv4Net};
    use rstest::rstest;
    use std::net::{IpAddr, Ipv4Addr};
    use tokio::time::Instant;

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
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: Instant::now(),
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
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: Instant::now(),
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
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: Instant::now(),
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
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: Instant::now(),
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
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: Instant::now(),
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
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: Instant::now(),
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
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: Instant::now(),
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
					best: false,
					reason: BestPathReason::NotBest,
					timestamp: Instant::now(),
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
        let mut builder = PathBuilder::builder(
            Ipv4Addr::new(1, 1, 1, 1),
            100,
            Ipv4Addr::new(2, 2, 2, 2),
            IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
            200,
        );
        for attr in attributes.into_iter() {
            builder.attr(attr).unwrap();
        }
        let paths: Vec<Path> = builder
            .nlri(nlri)
            .build()
            .unwrap()
            .into_iter()
            .map(|mut path| {
                path.id = 0;
                path
            })
            .collect();
        assert_eq!(expected, paths);
    }

    #[rstest(
		path,
		asn,
		expected,
		case(
			Path{
				id: 0,
				best: false,
				reason: BestPathReason::NotBest,
				timestamp: Instant::now(),
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
			65200,
			false,
		),
		case(
			Path{
				id: 0,
				best: false,
				reason: BestPathReason::NotBest,
				timestamp: Instant::now(),
				local_id: Ipv4Addr::new(1, 1, 1, 1),
				local_asn: 65200,
				peer_id: Ipv4Addr::new(2, 2, 2, 2),
				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
				peer_asn: 65100,
				family: AddressFamily::ipv4_unicast(),
				origin: Attribute::ORIGIN_IGP,
				local_pref: 0,
				med: 0,
				as_sequence: vec![65100, 65200],
				as_set: vec![],
				next_hops: vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))],
				propagate_attributes: Vec::new(),
				prefix: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 1, 0), 24).unwrap()),
			},
			65200,
			true,
		),
		case(
			Path{
				id: 0,
				best: false,
				reason: BestPathReason::NotBest,
				timestamp: Instant::now(),
				local_id: Ipv4Addr::new(1, 1, 1, 1),
				local_asn: 100,
				peer_id: Ipv4Addr::new(2, 2, 2, 2),
				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
				peer_asn: 200,
				family: AddressFamily::ipv4_unicast(),
				origin: Attribute::ORIGIN_IGP,
				local_pref: 0,
				med: 0,
				as_sequence: vec![200],
				as_set: vec![65100, 65200],
				next_hops: vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))],
				propagate_attributes: Vec::new(),
				prefix: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 1, 0), 24).unwrap()),
			},
			100,
			false,
		),
		case(
			Path{
				id: 0,
				best: false,
				reason: BestPathReason::NotBest,
				timestamp: Instant::now(),
				local_id: Ipv4Addr::new(1, 1, 1, 1),
				local_asn: 100,
				peer_id: Ipv4Addr::new(2, 2, 2, 2),
				peer_addr: IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)),
				peer_asn: 200,
				family: AddressFamily::ipv4_unicast(),
				origin: Attribute::ORIGIN_IGP,
				local_pref: 0,
				med: 0,
				as_sequence: vec![200],
				as_set: vec![65100, 65200],
				next_hops: vec![IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))],
				propagate_attributes: Vec::new(),
				prefix: IpNet::V4(Ipv4Net::new(Ipv4Addr::new(10, 10, 1, 0), 24).unwrap()),
			},
			65100,
			true,
		),
	)]
    fn works_has_as(path: Path, asn: u32, expected: bool) {
        assert_eq!(expected, path.has_as(asn));
    }
}
