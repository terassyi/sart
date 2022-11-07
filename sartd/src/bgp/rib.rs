use std::{collections::HashMap, ops::Add};
use futures::FutureExt;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use ipnet::IpNet;

use super::{path::Path, family::AddressFamily, error::{Error, RibError}, event::RibEvent, config::NeighborConfig, peer::neighbor::NeighborPair};

#[derive(Debug)]
pub(crate) struct Table {
	inner: HashMap<IpNet, Vec<Path>>,
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
		match self.inner.get_mut(&prefix) {
			Some(paths) => paths.push(path),
			None => { self.inner.insert(prefix, vec![path]); },
		}
		self.received += 1;
	}

	fn remove(&mut self, prefix: &IpNet) {
		let d = self.inner.remove(prefix);
		match d {
			Some(d) => self.dropped += d.len(),
			None => {},
		}
	}

	fn get(&self, prefix: &IpNet) -> Option<&Vec<Path>> {
		self.inner.get(prefix)
	}

	fn get_mut(&mut self, prefix: &IpNet) -> Option<&mut Vec<Path>> {
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

	pub fn get(&self, family: &AddressFamily, prefix: &IpNet) -> Option<&Vec<Path>> {
		match self.table.get(family) {
			Some(table) => table.get(prefix),
			None => None,
		}
	}

	pub fn get_mut(&mut self, family: &AddressFamily, prefix: &IpNet) -> Option<&mut Vec<Path>> {
		match self.table.get_mut(family) {
			Some(table) => table.get_mut(prefix),
			None => None,
		}
	}

	pub fn prefixes(&self, family: &AddressFamily) ->  Option<std::collections::hash_map::Keys<'_, IpNet, Vec<Path>, >> {
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
	table: Table,
}

impl LocRib {
	pub fn new() -> Self {
		Self {
			table: Table::new(),
		}
	}
}

#[derive(Debug)]
pub(crate) struct RibManager {
	loc_rib: LocRib,
	endpoint: String,
	event_rx: UnboundedReceiver<RibEvent>,
	peers_tx: HashMap<NeighborPair, UnboundedSender<RibEvent>>,
}

impl RibManager {
	pub fn new(endpoint: String) -> (Self, UnboundedSender<RibEvent>) {
		let (tx, rx) = unbounded_channel::<RibEvent>();
		(Self {
			loc_rib: LocRib::new(),
			endpoint,
			event_rx: rx,
			peers_tx: HashMap::new(),
		}, tx)
	}

	pub fn add_peer(&mut self, neighbor: NeighborPair, tx: UnboundedSender<RibEvent>) -> Result<(), RibError> {
		match self.peers_tx.get(&neighbor) {
			Some(_) => Err(RibError::PeerAlreadyRegistered),
			None => {
				self.peers_tx.insert(neighbor, tx);
				Ok(())
			}
		}
	}

	pub async fn serve(&mut self) {
		loop {
			futures::select_biased! {
				event = self.event_rx.recv().fuse() => {

				}
			}
		}
	}


}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::unbounded_channel;
	use std::net::{IpAddr, Ipv4Addr};

    use crate::bgp::{event::RibEvent, peer::neighbor::NeighborPair};

    use super::{Table, RibManager};

	#[test]
	fn works_table() {
		let mut table = Table::new();
	}

	#[test]
	fn works_rib_manager_add_peer() {
		let (mut manager, event_tx) = RibManager::new("test_endpoint".to_string());
		let (tx, rx) = unbounded_channel::<RibEvent>();
		manager.add_peer(NeighborPair::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 100), tx).unwrap();
	}
}
