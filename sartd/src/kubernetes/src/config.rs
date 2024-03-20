use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Deserialize, Serialize)]
pub struct Tls {
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum Mode {
    LB,
    CNI,
    #[default]
    Dual,
}

impl std::fmt::Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CNI => write!(f, "cni"),
            Self::LB => write!(f, "lb"),
            Self::Dual => write!(f, "dual"),
        }
    }
}

#[derive(Debug, Clone, Copy, Error)]
pub struct ParseModeError;

impl std::fmt::Display for ParseModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        "provided string was not `true` or `false`".fmt(f)
    }
}

impl FromStr for Mode {
    type Err = ParseModeError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "lb" => Ok(Mode::LB),
            "cni" => Ok(Mode::CNI),
            "dual" => Ok(Mode::Dual),
            _ => Err(ParseModeError),
        }
    }
}
