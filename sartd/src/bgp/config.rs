use serde::{Deserialize, Serialize};
use std::fs;
use std::net::Ipv4Addr;

use crate::bgp::error::*;
use crate::bgp::server::Bgp;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct Config {
    pub port: u16,
    pub as_number: u32,
    pub router_id: Ipv4Addr,
}

impl Config {
    pub fn default() -> Self {
        Config {
            port: Bgp::BGP_PORT,
            as_number: 0,
            router_id: Ipv4Addr::new(1, 1, 1, 1),
        }
    }

    pub fn load(file: &str) -> Result<Self, Error> {
        let contents = fs::read_to_string(file).map_err(|e| Error::StdIoErr(e))?;
        serde_yaml::from_str(&contents).map_err(|_| Error::Config(ConfigError::FailedToLoad))
    }

    pub fn set_as_number(&mut self, asn: u32) {
        self.as_number = asn;
    }
    pub fn set_router_id(&mut self, router_id: Ipv4Addr) {
        self.router_id = router_id;
    }
    pub fn set_local_port(&mut self, port: u16) {
        self.port = port;
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use std::net::Ipv4Addr;
    #[test]
    fn work_serd_yaml_from_str() {
        let yaml_str = r"port: 179
as_number: 6550
router_id: 1.1.1.1
";
        let conf: Config = serde_yaml::from_str(yaml_str).unwrap();
        assert_eq!(179, conf.port);
        assert_eq!(6550, conf.as_number);
        assert_eq!(Ipv4Addr::new(1, 1, 1, 1), conf.router_id);
    }
}
