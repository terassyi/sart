use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};

use super::error::*;

use super::event::ControlEvent;
use super::packet::attribute::Attribute;
use super::server::Bgp;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub asn: u32,
    pub router_id: Ipv4Addr,
    pub fib_endpoint: Option<String>,
    pub fib_table: Option<u8>,
    pub exporter: Option<String>,
    pub neighbors: Vec<NeighborConfig>,
    pub multi_path: Option<bool>,
    pub paths: Option<Vec<PathConfig>>,
}

impl Config {
    pub fn default() -> Self {
        Config {
            asn: 0,
            fib_endpoint: None,
            fib_table: Some(Bgp::ROUTE_TABLE_MAIN),
            exporter: None,
            router_id: Ipv4Addr::new(0, 0, 0, 0),
            neighbors: Vec::new(),
            multi_path: Some(false),
            paths: None,
        }
    }

    pub fn load(file: &str) -> Result<Self, Error> {
        let contents = fs::read_to_string(file).map_err(Error::StdIoErr)?;
        serde_yaml::from_str(&contents).map_err(|_| Error::Config(ConfigError::FailedToLoad))
    }

    pub fn set_as_number(&mut self, asn: u32) {
        self.asn = asn;
    }

    pub fn set_router_id(&mut self, router_id: Ipv4Addr) {
        self.router_id = router_id;
    }
    pub fn get_control_event(&self) -> Vec<ControlEvent> {
        self.into()
    }
}

impl Into<Vec<ControlEvent>> for &Config {
    fn into(self) -> Vec<ControlEvent> {
        let mut events = Vec::new();
        for neighbor in self.neighbors.iter() {
            events.push(ControlEvent::AddPeer(neighbor.clone()));
        }
        if let Some(paths) = &self.paths {
            for path in paths.iter() {
                let prefixes = path.prefixes.iter().map(|p| p.parse().unwrap()).collect();
                let attributes = match &path.attributes {
                    Some(attrs) => attrs
                        .iter()
                        .map(|a| Attribute::from_str_pair(a.0, a.1).unwrap())
                        .collect(),
                    None => Vec::new(),
                };
                events.push(ControlEvent::AddPath(prefixes, attributes))
            }
        }
        events
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct NeighborConfig {
    pub name: String,
    pub asn: u32,
    pub address: IpAddr,
    pub router_id: Ipv4Addr,
    pub passive: Option<bool>,
}

impl TryFrom<&sartd_proto::sart::Peer> for NeighborConfig {
    type Error = Error;
    fn try_from(value: &sartd_proto::sart::Peer) -> Result<Self, Self::Error> {
        let addr: IpAddr = match value.address.parse() {
            Ok(a) => a,
            Err(e) => return Err(Error::Config(ConfigError::InvalidArgument(e.to_string()))),
        };
        let id: Ipv4Addr = match value.router_id.parse() {
            Ok(id) => id,
            Err(e) => return Err(Error::Config(ConfigError::InvalidArgument(e.to_string()))),
        };
        Ok(NeighborConfig {
            name: value.name.clone(),
            asn: value.asn,
            address: addr,
            router_id: id,
            passive: Some(value.passive_open),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathConfig {
    pub prefixes: Vec<String>,
    pub attributes: Option<HashMap<String, String>>,
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::net::Ipv4Addr;
    #[test]
    fn work_serd_yaml_from_str() {
        let yaml_str = r"asn: 6550
router_id: 1.1.1.1
fib_endpoint: test
neighbors:
  - asn: 100
    name: test
    router_id: 2.2.2.2
    address: 2.2.2.2
  - asn: 200
    name: test2
    router_id: 3.3.3.3
    address: '::1'
    passive: true
paths:
  - prefixes:
      - 10.0.0.0/24
      - 10.1.0.0/24
    attributes:
      origin: 1
";
        let conf: Config = serde_yaml::from_str(yaml_str).unwrap();
        assert_eq!(6550, conf.asn);
        assert_eq!(Ipv4Addr::new(1, 1, 1, 1), conf.router_id);
    }
}
