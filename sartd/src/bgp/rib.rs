use std::{collections::HashMap, ops::Add, sync::Arc};
use futures::FutureExt;
use tokio::sync::{mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender, Receiver, Sender, channel}, Notify};
use ipnet::IpNet;

use super::{path::Path, family::{AddressFamily, Afi}, error::{Error, RibError}, event::RibEvent, config::NeighborConfig, peer::neighbor::NeighborPair};

#[derive(Debug)]
pub(crate) struct Table {
	inner: HashMap<IpNet, Path>,
	received: usize,
	dropped: usize,
}

impl Table {
	fn new() -> Self {
		Self {
			inner: HashMap::new(),
			received: 0,
			dropped: 0,
		}
	}

	fn insert(&mut self, prefix: IpNet, path: Path) {
		self.inner.insert(prefix, path);
		self.received += 1;
	}

	fn remove(&mut self, prefix: &IpNet) {
		let d = self.inner.remove(prefix);
		self.dropped += 1;
	}

	fn get(&self, prefix: &IpNet) -> Option<&Path> {
		self.inner.get(prefix)
	}

	fn get_mut(&mut self, prefix: &IpNet) -> Option<&mut Path> {
		self.inner.get_mut(prefix)
	}
}


#[derive(Debug)]
pub(crate) struct AdjRibIn {
	table: HashMap<AddressFamily, Table>
}

impl AdjRibIn {
	pub fn new(families: Vec<AddressFamily>) -> Self {
		let mut table = HashMap::new();
		for family in families.into_iter() {
			table.insert(family, Table::new());
		}
		Self {
			table,
		}
	}

	pub fn add_reach(&mut self, family: AddressFamily) {
		match self.table.get(&family) {
			Some(_) => {},
			None => {self.table.insert(family, Table::new());},
		}
	}

	pub fn insert(&mut self, family: &AddressFamily, prefix: IpNet, path: Path) -> Result<(), RibError> {
		match self.table.get_mut(family) {
			Some(table) => {
				table.insert(prefix, path);
				Ok(())
			},
			None => Err(RibError::AddressFamilyNotSet),
		}
	}

	pub fn get(&self, family: &AddressFamily, prefix: &IpNet) -> Option<&Path> {
		match self.table.get(family) {
			Some(table) => table.get(prefix),
			None => None,
		}
	}

	pub fn get_mut(&mut self, family: &AddressFamily, prefix: &IpNet) -> Option<&mut Path> {
		match self.table.get_mut(family) {
			Some(table) => table.get_mut(prefix),
			None => None,
		}
	}

	pub fn prefixes(&self, family: &AddressFamily) ->  Option<std::collections::hash_map::Keys<'_, IpNet, Path, >> {
		match self.table.get(family) {
			Some(table) => Some(table.inner.keys()),
			None => None,
		}
	}

	pub fn remove(&mut self, family: &AddressFamily, prefix: &IpNet) {
		match self.table.get_mut(family) {
			Some(table) => table.remove(prefix),
			None => {},
		}
	}
}

#[derive(Debug)]
pub(crate) struct AdjRibOut {
	table: HashMap<AddressFamily, Table>
}

impl AdjRibOut {
	pub fn new(families: Vec<AddressFamily>) -> Self {
		let mut table = HashMap::new();
		for family in families.into_iter() {
			table.insert(family, Table::new());
		}
		Self {
			table,
		}
	}
}

#[derive(Debug)]
pub(crate) struct LocRib {
	table: HashMap<Afi, HashMap<IpNet, Vec<Path>>>,
	received: usize,
	dropped: usize,
}

impl LocRib {
	pub fn new(protocols: Vec<Afi>) -> Self {
		let mut loc_rib = Self {
			table: HashMap::new(),
			received: 0,
			dropped: 0,
		};
		for protocol in protocols.into_iter() {
			loc_rib.table.insert(protocol, HashMap::new());
		}
		loc_rib
	}

	fn set_protocol(&mut self, protocol: Afi) -> Result<(), Error> {
		match self.table.get(&protocol) {
			Some(_) => Err(Error::Rib(RibError::ProtocolIsAlreadyRegistered)),
			None => {
				self.table.insert(protocol, HashMap::new());
				Ok(())
			}
		}
	}
}

#[derive(Debug)]
pub(crate) struct RibManager {
	loc_rib: LocRib,
	endpoint: String,
	peers_tx: HashMap<NeighborPair, Sender<RibEvent>>,
}

impl RibManager {
	pub fn new(endpoint: String, protocols: Vec<Afi>) -> Self {
		Self {
			loc_rib: LocRib::new(protocols),
			endpoint,
			peers_tx: HashMap::new(),
		}
	}

	#[tracing::instrument(skip(self))]
	pub fn handle(&mut self, event: RibEvent) -> Result<(), Error> {
		match event {
			RibEvent::AddPeer{neighbor, rib_event_tx} => self.add_peer(neighbor, rib_event_tx),
		}
	}

	#[tracing::instrument(skip(self,tx))]
	fn add_peer(&mut self, neighbor: NeighborPair, tx: Sender<RibEvent>) -> Result<(), Error> {
		tracing::info!(level="rib",event="AddPeer");
		match self.peers_tx.get(&neighbor) {
			Some(_) => Err(Error::Rib(RibError::PeerAlreadyRegistered)),
			None => {
				self.peers_tx.insert(neighbor, tx);
				Ok(())
			}
		}
	}

	fn add_network(&mut self, network: String) -> Result<(), Error> {
		Ok(())
	}
}

#[cfg(test)]
mod tests {
    use ipnet::{IpNet, Ipv4Net};
    use tokio::sync::mpsc::{unbounded_channel, channel};
	use std::net::{IpAddr, Ipv4Addr};
	use rstest::rstest;

    use crate::bgp::{event::RibEvent, peer::neighbor::NeighborPair, family::{Afi, AddressFamily}, path::Path, packet::attribute::Attribute};
	use crate::bgp::packet::message::Message;
	use crate::bgp::packet::prefix::Prefix;
	use crate::bgp::packet::attribute::{Base, ASSegment};

    use super::{Table, RibManager};

	#[test]
	fn works_table() {
		let mut table = Table::new();
	}

	#[test]
	fn works_rib_manager_add_peer() {
		let mut manager = RibManager::new("test_endpoint".to_string(), vec![Afi::IPv4]);
		let (tx, _rx) = channel::<RibEvent>(128);
		manager.add_peer(NeighborPair::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 100), tx).unwrap();
	}

	#[rstest(
		msg,
		expected_received,
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
			1,
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
			1,
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
			3,
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
			3,
		),
	)]
	fn works_table_insert_and_remove(msg: Message, expected_received: usize) {
		let (_withdrawn_routes, attributes, nlri) = msg.to_update().unwrap();
		let mut builder = crate::bgp::path::PathBuilder::builder(Ipv4Addr::new(1, 1, 1, 1), 100, Ipv4Addr::new(2, 2, 2, 2), IpAddr::V4(Ipv4Addr::new(2, 2, 2, 2)), 200);
		for attr in attributes.into_iter() {
			builder.attr(attr).unwrap();
		}
		let paths: Vec<Path> = builder.nlri(nlri)
				.build().unwrap();
		let mut table = Table::new();
		for path in paths.into_iter() {
			table.insert(path.prefix(), path);
		}
		assert_eq!(expected_received, table.received);
	}
}
