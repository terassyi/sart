use std::net::IpAddr;

use schemars::JsonSchema;
use serde::{Serialize, Deserialize};

use crate::error::Error;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Info {
	pub asn: u32,
	pub router_id: IpAddr,
	pub port: u32,
}

impl TryFrom<&crate::proto::sart::BgpInfo> for Info {
	type Error = Error;
	fn try_from(value: &crate::proto::sart::BgpInfo) -> Result<Self, Self::Error> {
		let addr: IpAddr = value.router_id.parse().map_err(|_| Error::InvalidParameter("router_id".to_string()))?;
		Ok(Self {
			asn: value.asn,
			router_id: addr,
			port: value.port,
		})
	}
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) enum Protocol {
	#[serde(rename = "unknown")]
	Unknown,
	#[default]
	#[serde(rename = "ipv4")]
	IPv4,
	#[serde(rename = "ipv6")]
	IPv6,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) enum Safi {
	#[default]
	#[serde(rename = "unicast")]
	Unicast,
	#[serde(rename = "multicast")]
	Multicast,
}

impl TryFrom<i32> for Protocol {
	type Error = Error;
	fn try_from(value: i32) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Protocol::Unknown),
			1 => Ok(Protocol::IPv4),
			2 => Ok(Protocol::IPv6),
			_ => Err(Error::InvalidProtocol)
		}
	}
}
