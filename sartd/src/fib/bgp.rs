
use serde::{Deserialize, Serialize};

use super::error::Error;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Bgp {
	pub endpoint: String,
}

impl Bgp {
	pub fn new(endpoint: String) -> Self {
		Self { endpoint }
	}

	pub fn subscribe(&self) -> Result<(), Error> {
		Ok(())
	}

	pub fn publish(&self) -> Result<(), Error> {
		Ok(())
	}
}
