use std::default;

use schemars::JsonSchema;
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) enum Type {
	#[default]
	Pod,
	LoadBalancer,
}


// Bgp attributes

pub(crate) type Origin = String;
pub(crate) type  LocalPref = u32;

#[derive(Debug, Serialize, Deserialize, Default, Clone, JsonSchema)]
pub(crate) enum Status {
	#[default]
	NotAdvertised,
	Advertised,
	Updating,
}
