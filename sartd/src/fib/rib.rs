use std::{collections::HashMap, net::IpAddr};
use ipnet::{IpNet, IpAdd};

use crate::fib::error::*;

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone)]
pub(crate) struct Route {
	pub destination: IpNet,
	pub version: rtnetlink::IpVersion,
	pub protocol: Protocol,
	pub scope: Scope,
	pub typ: Type,
	pub next_hops: Vec<IpAddr>,
	pub source: Option<IpAddr>,
	pub priority: u32,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum Protocol {
	Unspec = 0,
	Redirect = 1,
	Kernel = 2,
	Boot = 3,
	Static = 4,
	Bgp = 186,
	IsIs = 187,
	Ospf = 188,
	Rip = 189,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum Scope {
	Universe = 0,
	Site = 200,
	Link = 253,
	Host = 254,
	Nowhere = 255,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum Type {
	UnspecType = 0,
	Unicast = 1,
	Local = 2,
	Broadcast = 3,
	Anycast = 4,
	Multicast = 5,
	Blackhole = 6,
	Unreachable = 7,
	Prohibit = 8,
	Throw = 9,
	Nat = 10,
}

#[derive(Debug)]
pub(crate) struct Rib {
	pub ip_version: rtnetlink::IpVersion,
	pub table: Table,
}

impl Default for Rib {
	fn default() -> Self {
		Self { 
			ip_version: rtnetlink::IpVersion::V4,
			table: Table { inner: HashMap::new() }
		}
	}
}

#[derive(Debug)]
pub(crate) struct Table {
	inner: HashMap<IpNet, Vec<Route>>
}

impl Rib {
	pub fn new(protocol: rtnetlink::IpVersion) -> Self {
		Self { 
			ip_version: protocol,
			table: Table {
				inner: HashMap::new(),
			}
		}
	}

	pub fn insert(&mut self, destination: IpNet, route: Route) -> Option<Route> {
		if route.version != self.ip_version {
			return None;
		}
		let ret_route = route.clone();
		match self.table.inner.get_mut(&destination) {
			Some(routes) => {
				routes.push(route);
			},
			None => {
				let routes = vec![route];
				self.table.inner.insert(destination, routes);
			}
		}
		Some(ret_route)
	}

	pub fn get(&self, destination: &IpNet) -> Option<&Vec<Route>> {
		self.table.inner.get(destination)
	}

}
