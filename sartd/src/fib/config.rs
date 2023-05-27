use std::fs;

use super::{channel::Channel, error::*};
use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Config {
	pub endpoint: String,
	pub channels: Vec<Channel>,
}

impl Config {
	pub fn load(file: &str) -> Result<Self, Error> {
		let contents = fs::read_to_string(file).map_err(Error::StdIoErr)?;
        serde_yaml::from_str(&contents).map_err(|_| Error::Config(ConfigError::FailedToLoad))
	}
}

#[cfg(test)]
mod tests {
	use super::Config;
	#[test]
	fn works_serd_yalm_from_str() {
		let yaml_str = r"endpoint: localhost:5001
channels:
- name: kernel_tables
  ip_version: ipv4
  subscribers:
    - protocol: kernel
      tables:
      - 254
      - 8
  publishers:
    - protocol: bgp
      endpoint: localhost:5000
- name: bgp_rib
  ip_version: ipv4
  subscribers:
    - protocol: bgp
      endpoint: localhost:5000
  publishers:
    - protocol: kernel
      tables:
      - 254
";
	let conf: Config = serde_yaml::from_str(yaml_str).unwrap();
	}
}
