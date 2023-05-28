use std::{collections::HashMap, net::IpAddr, ops::Index};
use ipnet::{IpNet, IpAdd, Ipv4Net};

use crate::{fib::error::*, proto};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum RequestType {
    AddRoute,
    DeleteRoute,
    AddMultiPathRoute,
    DeleteMultiPathRoute,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone)]
pub(crate) struct Route {
	pub destination: IpNet,
	pub version: rtnetlink::IpVersion,
	pub protocol: Protocol,
	pub scope: Scope,
	pub typ: Type,
	pub next_hops: Vec<NextHop>,
	pub source: Option<IpAddr>,
	pub priority: u32,
}

impl Default for Route {
	fn default() -> Self {
		Self {
    		destination: "0.0.0.0/0".parse().unwrap(),
    		version: rtnetlink::IpVersion::V4,
    		protocol: Protocol::Unspec,
    		scope: Scope::Universe,
    		typ: Type::UnspecType,
    		next_hops: Vec::new(),
    		source: None,
    		priority: 0,
		}
	}
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone)]
pub(crate) struct NextHop {
	pub gateway: IpAddr,
	pub weight: u32,
	pub flags: NextHopFlags,
	pub interface: u32,
}

impl TryFrom<&proto::sart::NextHop> for NextHop {
	type Error = Error;
	fn try_from(value: &proto::sart::NextHop) -> Result<Self, Self::Error> {
		let gateway = value.gateway.parse().map_err(|_| Error::FailedToParseAddress)?;
		
		Ok(NextHop { 
			gateway, 
			weight: value.weight, 
			flags: NextHopFlags::try_from(value.flags)?, 
			interface: value.interface
		})
	}
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone)]
pub(crate) enum NextHopFlags {
	Empty = 0,
	Dead = 1,
	Pervasive = 2,
	Onlink = 3,
	Offload = 4,
	Linkdown = 16,
	Unresolved = 32,
}

impl TryFrom<i32> for NextHopFlags {
	type Error = Error;
	fn try_from(value: i32) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(NextHopFlags::Empty),
			1 => Ok(NextHopFlags::Dead),
			2 => Ok(NextHopFlags::Pervasive),
			3 => Ok(NextHopFlags::Onlink),
			4 => Ok(NextHopFlags::Offload),
			16 => Ok(NextHopFlags::Linkdown),
			32 => Ok(NextHopFlags::Unresolved),
			_ => Err(Error::InvalidNextHopFlag)
		}
	}
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

impl TryFrom<i32> for Protocol {
	type Error = Error;
	fn try_from(value: i32) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Protocol::Unspec),
			1 => Ok(Protocol::Redirect),
			2 => Ok(Protocol::Kernel),
			3 => Ok(Protocol::Boot),
			4 => Ok(Protocol::Static),
			186 => Ok(Protocol::Bgp),
			187 => Ok(Protocol::IsIs),
			188 => Ok(Protocol::Ospf),
			189 => Ok(Protocol::Rip),
			_ => Err(Error::InvalidProtocol)
		}
	}
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub(crate) enum Scope {
	Universe = 0,
	Site = 200,
	Link = 253,
	Host = 254,
	Nowhere = 255,
}

impl TryFrom<i32> for Scope {
	type Error = Error;
	fn try_from(value: i32) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Scope::Universe),
			200 => Ok(Scope::Site),
			253 => Ok(Scope::Link),
			254 => Ok(Scope::Host),
			255 => Ok(Scope::Nowhere),
			_ => Err(Error::InvalidScope)
		}
	}
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

impl TryFrom<i32> for Type {
	type Error = Error;
	fn try_from(value: i32) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Type::UnspecType),
			1 => Ok(Type::Unicast),
			2 => Ok(Type::Local),
			3 => Ok(Type::Broadcast),
			4 => Ok(Type::Anycast),
			5 => Ok(Type::Multicast),
			6 => Ok(Type::Blackhole),
			7 => Ok(Type::Unreachable),
			8 => Ok(Type::Prohibit),
			9 => Ok(Type::Throw),
			10 => Ok(Type::Nat),
			_ => Err(Error::InvalidType)
		}	
	}
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

	pub fn remove(&mut self, route: Route) -> Option<Route> {
		if route.version != self.ip_version {
			return None;
		}
		match self.table.inner.get_mut(&route.destination) {
			Some(routes) => {
				if routes.is_empty() {
					return None
				}
				let index = match routes.iter().position(|r| {
					if r.next_hops.len() != route.next_hops.len() {
						return false
					}
					for i in 0..r.next_hops.len()-1 {
						if !r.next_hops[i].gateway.eq(&route.next_hops[i].gateway) {
							return false
						}
					}
					true
				}) {
					Some(index) => index,
					None => return None,
				};
				let ret_route = routes.remove(index);
				Some(ret_route)
			},
			None => {
				None
			}
		}
	}

}

impl TryFrom<&proto::sart::Route> for Route {
	type Error = Error;
	fn try_from(value: &proto::sart::Route) -> Result<Self, Self::Error> {
		let dst: IpNet = value.destination.parse().map_err(|_| Error::FailedToParseAddress)?;
		let ip_version = match value.version {
			2 => rtnetlink::IpVersion::V4,
			10 => rtnetlink::IpVersion::V6,
			_ => return Err(Error::InvalidIpVersion)
		};
		let mut next_hops = Vec::new();
		for n in value.next_hops.iter() {
			next_hops.push(NextHop::try_from(n)?);
		}
		let source = match value.source.parse() {
			Ok(a) => Some(a),
			Err(_) => None,
		};

		Ok(Route {
    		destination: dst,
    		version: ip_version,
    		protocol: Protocol::try_from(value.protocol)?,
    		scope: Scope::try_from(value.scope)?,
    		typ: Type::try_from(value.r#type)?,
    		next_hops,
    		source,
    		priority: value.priority,
		})
	}
}
