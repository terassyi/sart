
use serde::{Deserialize, Serialize};

use super::error::Error;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Kernel {
	pub tables: Vec<u32>,
}

impl Kernel {
	pub fn new(table_ids: Vec<u32>) -> Self {
		Self { tables: table_ids }
	}

	pub fn subscribe(&self) -> Result<(), Error> {
		Ok(())
	}

	pub fn publish(&self) -> Result<(), Error> {
		Ok(())
	}
}
