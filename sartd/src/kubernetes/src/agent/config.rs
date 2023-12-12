use std::fs;

use serde::{Deserialize, Serialize};

use sartd_cert::constants::*;

use super::error::{ConfigError, Error};

pub const DEFAULT_ENDPOINT: &str = "0.0.0.0:5002";
pub const DEFAULT_PEER_STATE_WATCHER_ENDPOINT: &str = "0.0.0.0:5003";
pub const DEFAULT_REQUEUE_INTERVAL: u64 = 30 * 60;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub endpoint: String,
    pub tls: Tls,
    pub requeue_interval: u64,
    pub peer_state_watcher: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Tls {
    pub cert: String,
    pub key: String,
}

impl Config {
    pub fn load(file: &str) -> Result<Self, Error> {
        let contents = fs::read_to_string(file).map_err(Error::StdIo)?;
        serde_yaml::from_str(&contents).map_err(|_| Error::Config(ConfigError::FailedToLoad))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            endpoint: DEFAULT_ENDPOINT.to_string(),
            tls: Tls {
                cert: DEFAULT_TLS_CERT.to_string(),
                key: DEFAULT_TLS_KEY.to_string(),
            },
            requeue_interval: DEFAULT_REQUEUE_INTERVAL,
            peer_state_watcher: DEFAULT_PEER_STATE_WATCHER_ENDPOINT.to_string(),
        }
    }
}