use rscni::{error::Error, types::Args};
use serde::{Deserialize, Serialize};

const SART_CONFIG_ENDPOINT: &str = "endpoint";
const SART_CONFIG_TIMEOUT: &str = "timeout";

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Config {
    pub(crate) endpoint: String,
    pub(crate) timeout: u64,
}

impl Config {
    pub fn parse(conf: &Args) -> Result<Config, Error> {
        match &conf.config {
            Some(config) => {
                let endpoint = config
                    .custom
                    .get(SART_CONFIG_ENDPOINT)
                    .ok_or(Error::InvalidNetworkConfig(
                        "endpoint must be given.".to_string(),
                    ))?
                    .as_str()
                    .ok_or(Error::InvalidNetworkConfig(
                        "endpoint parameter must be given".to_string(),
                    ))?
                    .to_string();
                let timeout = config
                    .custom
                    .get(SART_CONFIG_TIMEOUT)
                    .ok_or(Error::InvalidNetworkConfig(
                        "timeout parameter must be given".to_string(),
                    ))?
                    .as_u64()
                    .ok_or(Error::InvalidNetworkConfig(
                        "timeout parameter must be number".to_string(),
                    ))?;

                Ok(Config { endpoint, timeout })
            }
            None => Err(Error::InvalidNetworkConfig(
                "configuration must be given from stdin".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use rscni::types::NetConf;
    use serde_json::json;

    use super::*;

    #[test]
    fn parse_config_from_net_conf() {
        let args = Args {
            config: Some(NetConf {
                custom: HashMap::from([
                    ("endpoint".to_string(), json!("test")),
                    ("timeout".to_string(), json!(60)),
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let conf = Config::parse(&args).unwrap();
        assert_eq!(
            Config {
                endpoint: "test".to_string(),
                timeout: 60,
            },
            conf
        );
    }
}
