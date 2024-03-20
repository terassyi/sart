use std::fs;

use serde::{Deserialize, Serialize};

use crate::config::{Mode, Tls};

use super::error::{ConfigError, Error};

use sartd_cert::constants::{DEFAULT_TLS_CERT, DEFAULT_TLS_KEY};

pub const DEFAULT_HTTP_PORT: u32 = 8080;
pub const DEFAULT_HTTPS_PORT: u32 = 8443;
pub const DEFAULT_ENDPOINT: &str = "0.0.0.0:5002";
pub const DEFAULT_REQUEUE_INTERVAL: u64 = 30 * 60;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub http_port: u32,
    pub https_port: u32,
    pub endpoint: String,
    pub tls: Tls,
    pub requeue_interval: u64,
    pub mode: Mode,
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
            http_port: DEFAULT_HTTP_PORT,
            https_port: DEFAULT_HTTPS_PORT,
            endpoint: DEFAULT_ENDPOINT.to_string(),
            tls: Tls {
                cert: DEFAULT_TLS_CERT.to_string(),
                key: DEFAULT_TLS_KEY.to_string(),
            },
            requeue_interval: DEFAULT_REQUEUE_INTERVAL,
            mode: Mode::default(),
        }
    }
}
