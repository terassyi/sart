use std::fs;

use serde::{Deserialize, Serialize};

use super::error::{ConfigError, Error};

pub(crate) const DEFAULT_ENDPOINT: &'static str = "0.0.0.0:5002";

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Config {
    pub endpoint: String,
}

impl Config {
    pub fn load(file: &str) -> Result<Self, Error> {
        let contents = fs::read_to_string(file).map_err(Error::StdIoError)?;
        serde_yaml::from_str(&contents).map_err(|_| Error::ConfigError(ConfigError::FailedToLoad))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            endpoint: DEFAULT_ENDPOINT.to_string(),
        }
    }
}
