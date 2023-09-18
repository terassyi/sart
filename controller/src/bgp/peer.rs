
use schemars::JsonSchema;
use serde::{Serialize, Deserialize};

pub(crate) const BGP_PORT: u32 = 179;

pub(crate) const DEFAULT_HOLD_TIME: u64 = 180;
pub(crate) const DEFAULT_KEEPALIVE_TIME: u64 = 90;


#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) struct Peer {
	pub name: Option<String>,
	pub local_asn: u32,
	pub local_addr: Option<String>,
	pub local_name: Option<String>,
	pub neighbor: Neighbor,
	pub hold_time: Option<u64>,
	pub keepalive_time: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) struct Neighbor {
	pub name: Option<String>,
	pub asn: u32,
	pub addr: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) enum Status {
	#[default]
	NotEstablished,
	Established,
}
