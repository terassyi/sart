
use std::default;

use schemars::JsonSchema;
use serde::{Serialize, Deserialize};

use crate::{proto, error::Error};

use super::common::Protocol;

pub(crate) const BGP_PORT: u32 = 179;

pub(crate) const DEFAULT_HOLD_TIME: u64 = 180;
pub(crate) const DEFAULT_KEEPALIVE_TIME: u64 = 90;


#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Peer {
	pub name: Option<String>,
	pub local_asn: u32,
	pub local_addr: Option<String>,
	pub local_name: Option<String>,
	pub neighbor: Neighbor,
	pub protocol: Protocol,
	pub hold_time: Option<u64>,
	pub keepalive_time: Option<u64>,
	pub state: Option<Status>,
}

impl TryFrom<&crate::proto::sart::Peer> for Peer {
	type Error = Error;
	fn try_from(value: &crate::proto::sart::Peer) -> Result<Self, Self::Error> {
		let protocol = Protocol::try_from(value.families.first().or(Some(&crate::proto::sart::AddressFamily{
			afi: crate::proto::sart::address_family::Afi::Ip4 as i32,
			safi: crate::proto::sart::address_family::Safi::Unicast as i32,
		}))
		.unwrap()
		.afi)?;

		Ok(Peer {
			name: None,
			local_asn: 0,
			local_addr: None,
			local_name: None,
			neighbor: Neighbor {
				name: None,
				asn: value.asn, 
				addr: value.address.clone(),
			},
			protocol: protocol,
			hold_time: Some(value.hold_time as u64),
			keepalive_time: Some(value.keepalive_time as u64),
			state: Some(Status::from(value.state)),
		})
	}

}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) struct Neighbor {
	pub name: Option<String>,
	pub asn: u32,
	pub addr: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema, PartialEq, Eq)]
pub(crate) enum Status {
	#[default]
	NotEstablished,
	Idle,
	Active,
	Connect,
	OpenSent,
	OpenConfirm,
	Established,
}

impl From<crate::proto::sart::peer::State> for Status {
	fn from(value: crate::proto::sart::peer::State) -> Self {
		match value {
			crate::proto::sart::peer::State::Unknown => Status::NotEstablished,
			crate::proto::sart::peer::State::Idle => Status::Idle,
			crate::proto::sart::peer::State::Active => Status::Active,
			crate::proto::sart::peer::State::Connect => Status::Connect,
			crate::proto::sart::peer::State::OpenSent => Status::OpenSent,
			crate::proto::sart::peer::State::OpenConfirm => Status::OpenConfirm,
			crate::proto::sart::peer::State::Established => Status::Established,
		}
	}
}

impl From<i32> for Status {
	fn from(value: i32) -> Self {
		match value {
			1 => Status::Idle,
			2 => Status::Connect,
			3 => Status::Active,
			4 => Status::OpenSent,
			5 => Status::OpenConfirm,
			6 => Status::Established,
			_ => Status::NotEstablished,
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) enum SpeakerType {
	#[default]
	#[serde(rename= "sart")]
	Sart,
}
