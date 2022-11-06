use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::fs;
use std::net::{IpAddr, Ipv4Addr};

use crate::bgp::error::*;
use crate::bgp::server::Bgp;

use super::capability::Capability;
use super::event::ControlEvent;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Config {
    pub asn: u32,
    pub router_id: Ipv4Addr,
    pub neighbors: Vec<NeighborConfig>,
}

impl Config {
    pub fn default() -> Self {
        Config {
            asn: 0,
            router_id: Ipv4Addr::new(1, 1, 1, 1),
            neighbors: Vec::new(),
        }
    }

    pub fn load(file: &str) -> Result<Self, Error> {
        let contents = fs::read_to_string(file).map_err(|e| Error::StdIoErr(e))?;
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
        events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct NeighborConfig {
    pub asn: u32,
    pub address: IpAddr,
    pub router_id: Ipv4Addr,
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::net::Ipv4Addr;
    #[test]
    fn work_serd_yaml_from_str() {
        let yaml_str = r"asn: 6550
router_id: 1.1.1.1
neighbors:
  - asn: 100
    router_id: 2.2.2.2
    address: 2.2.2.2
  - asn: 200
    router_id: 3.3.3.3
    address: '::1'
";
        let conf: Config = serde_yaml::from_str(yaml_str).unwrap();
        assert_eq!(6550, conf.asn);
        assert_eq!(Ipv4Addr::new(1, 1, 1, 1), conf.router_id);
    }
}
