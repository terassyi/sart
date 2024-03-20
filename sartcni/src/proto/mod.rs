use rscni::{
    error::Error,
    types::{Args, CNIResult, Dns, Interface, IpConfig, Route},
};
use serde::{Deserialize, Serialize};

use crate::error::{ERROR_CODE_GRPC, ERROR_MSG_GRPC};

pub mod sart {
    include!("sart.v1.rs");
}
pub mod google {
    pub mod protobuf {
        include!("google.protobuf.rs");
    }
}

pub async fn connect(
    endpoint: &str,
) -> Result<sart::cni_api_client::CniApiClient<tonic::transport::Channel>, Error> {
    sart::cni_api_client::CniApiClient::connect(endpoint.to_string())
        .await
        .map_err(|e| Error::Custom(ERROR_CODE_GRPC, ERROR_MSG_GRPC.to_string(), e.to_string()))
}

impl From<&Args> for sart::Args {
    fn from(args: &Args) -> Self {
        Self {
            container_id: args.container_id.clone(),
            netns: args
                .netns
                .clone()
                .and_then(|netns| netns.as_os_str().to_str().map(|s| s.to_string()))
                .unwrap_or_default(),
            ifname: args.ifname.clone(),
            path: args
                .path
                .iter()
                .filter_map(|p| p.as_os_str().to_str().map(|s| s.to_string()))
                .collect(),
            args: args.args.clone().unwrap_or_default(),
            prev_result: None,
            conf: None,
            data: String::new(),
        }
    }
}

impl From<&CNIResult> for sart::CniResult {
    fn from(res: &CNIResult) -> Self {
        Self {
            interfaces: res.interfaces.iter().map(sart::Interface::from).collect(),
            ips: res.ips.iter().map(sart::IpConf::from).collect(),
            routes: res.routes.iter().map(sart::RouteConf::from).collect(),
            dns: res.dns.as_ref().map(sart::Dns::from),
        }
    }
}

impl From<&sart::CniResult> for CNIResult {
    fn from(res: &sart::CniResult) -> Self {
        Self {
            interfaces: res.interfaces.iter().map(Interface::from).collect(),
            ips: res.ips.iter().map(IpConfig::from).collect(),
            routes: res.routes.iter().map(Route::from).collect(),
            dns: res.dns.clone().map(|d| Dns::from(&d)),
        }
    }
}

impl From<&Interface> for sart::Interface {
    fn from(iface: &Interface) -> Self {
        Self {
            name: iface.name.clone(),
            mac: iface.mac.clone(),
            sandbox: iface.sandbox.clone().unwrap_or_default(),
        }
    }
}

impl From<&sart::Interface> for Interface {
    fn from(iface: &sart::Interface) -> Self {
        Self {
            name: iface.name.clone(),
            mac: iface.mac.clone(),
            sandbox: if iface.sandbox.is_empty() {
                None
            } else {
                Some(iface.sandbox.clone())
            },
        }
    }
}

impl From<&IpConfig> for sart::IpConf {
    fn from(ip: &IpConfig) -> Self {
        Self {
            interface: ip.interface.unwrap_or_default(),
            address: ip.address.clone(),
            gateway: ip.gateway.clone().unwrap_or_default(),
        }
    }
}

impl From<&sart::IpConf> for IpConfig {
    fn from(ip: &sart::IpConf) -> Self {
        Self {
            interface: if ip.interface == 0 {
                None
            } else {
                Some(ip.interface)
            },
            address: ip.address.clone(),
            gateway: if ip.gateway.is_empty() {
                None
            } else {
                Some(ip.gateway.clone())
            },
        }
    }
}

impl From<&Route> for sart::RouteConf {
    fn from(route: &Route) -> Self {
        Self {
            dst: route.dst.clone(),
            gw: route.gw.clone().unwrap_or_default(),
            mtu: route.mtu.unwrap_or_default() as i32,
            advmss: route.advmss.unwrap_or_default() as i32,
        }
    }
}

impl From<&sart::RouteConf> for Route {
    fn from(route: &sart::RouteConf) -> Self {
        Self {
            dst: route.dst.clone(),
            gw: if route.gw.is_empty() {
                None
            } else {
                Some(route.gw.clone())
            },
            mtu: if route.mtu == 0 {
                None
            } else {
                Some(route.mtu as u32)
            },
            advmss: if route.advmss == 0 {
                None
            } else {
                Some(route.advmss as u32)
            },
        }
    }
}

impl From<&Dns> for sart::Dns {
    fn from(dns: &Dns) -> Self {
        Self {
            nameservers: dns.nameservers.clone(),
            domain: dns.domain.clone().unwrap_or_default(),
            search: dns.search.clone().unwrap_or_default(),
            options: dns.options.clone().unwrap_or_default(),
        }
    }
}

impl From<&sart::Dns> for Dns {
    fn from(dns: &sart::Dns) -> Self {
        Self {
            nameservers: dns.nameservers.clone(),
            domain: if dns.domain.is_empty() {
                None
            } else {
                Some(dns.domain.clone())
            },
            search: if dns.search.is_empty() {
                None
            } else {
                Some(dns.search.clone())
            },
            options: if dns.options.is_empty() {
                None
            } else {
                Some(dns.options.clone())
            },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct CNIErrorDetail {
    pub(super) code: u32,
    pub(super) msg: String,
    pub(super) details: String,
}

impl From<&CNIErrorDetail> for Error {
    fn from(res: &CNIErrorDetail) -> Self {
        if res.code > 100 {
            return Error::Custom(res.code, res.msg.clone(), res.details.clone());
        }
        match res.code {
            1 => Error::IncompatibleVersion(res.details.clone()),
            2 => Error::UnsupportedNetworkConfiguration(res.details.clone()),
            3 => Error::NotExist(res.details.clone()),
            4 => Error::InvalidEnvValue(res.details.clone()),
            5 => Error::IOFailure(res.details.clone()),
            6 => Error::FailedToDecode(res.details.clone()),
            7 => Error::InvalidNetworkConfig(res.details.clone()),
            11 => Error::TryAgainLater(res.details.clone()),
            _ => Error::FailedToDecode(format!("unknown error code: {}", res.code)),
        }
    }
}

impl From<&Error> for sart::ErrorResult {
    fn from(err: &Error) -> Self {
        Self {
            code: u32::from(err) as i32,
            msg: err.to_string(),
            details: err.details(),
        }
    }
}

impl From<&sart::ErrorResult> for Error {
    fn from(res: &sart::ErrorResult) -> Self {
        if res.code > 100 {
            return Error::Custom(res.code as u32, res.msg.clone(), res.details.clone());
        }
        match res.code {
            1 => Error::IncompatibleVersion(res.details.clone()),
            2 => Error::UnsupportedNetworkConfiguration(res.details.clone()),
            3 => Error::NotExist(res.details.clone()),
            4 => Error::InvalidEnvValue(res.details.clone()),
            5 => Error::IOFailure(res.details.clone()),
            6 => Error::FailedToDecode(res.details.clone()),
            7 => Error::InvalidNetworkConfig(res.details.clone()),
            11 => Error::TryAgainLater(res.details.clone()),
            _ => Error::FailedToDecode(format!("unknown error code: {}", res.code)),
        }
    }
}
