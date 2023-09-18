use std::default;

use schemars::JsonSchema;
use serde::{Serialize, Deserialize};


#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) struct Peer {
	pub name: String,
	pub local_asn: u32,
	pub local_addr: String,
	pub neighbors: Vec<Neighbor>,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) struct Neighbor {
	pub name: String,
	pub asn: u32,
	pub addr: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) enum Status {
	#[default]
	NotEstablished,
	Established,
}
