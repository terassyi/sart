use crate::fib::bgp;
use crate::fib::kernel;
use serde::{Deserialize, Serialize};

use super::rib::Rib;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Channel {
	pub name: String,
	pub ip_version: String,
	pub publishers: Vec<Protocol>,
	pub subscribers: Vec<Protocol>,
	#[serde(skip)]
	rib: Rib,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "protocol")]
pub(crate) enum Protocol {
	#[serde(rename = "kernel")]
	Kernel(kernel::Kernel),
	#[serde(rename = "bgp")]
	Bgp(bgp::Bgp),
}

impl Channel {
	pub fn run(&mut self) {

	}
}
