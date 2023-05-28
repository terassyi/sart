
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::Receiver;

use super::{error::Error, rib::{RequestType, Route}};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Kernel {
	pub tables: Vec<u32>,
}

impl Kernel {
	pub fn new(table_ids: Vec<u32>) -> Self {
		Self { tables: table_ids }
	}

	pub async fn subscribe(&self) -> Result<Receiver<(RequestType, Route)>, Error> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<(RequestType, Route)>(128);
		Ok(rx)
	}

	pub async fn publish(&self) -> Result<(), Error> {
		Ok(())
	}
}
