use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use k8s_openapi::api::core::v1::Node;
use kube::{Resource, runtime::controller::Action};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::time::Duration;

use crate::error::Error;
use crate::context::Context;
use crate::speaker;
use crate::speaker::sart::SartSpeaker;
use crate::speaker::speaker::{Speaker, DEFAULT_ENDPOINT_CONNECT_TIMEOUT};

use super::clusterbgp::ClusterBgp;


pub(crate) const DEFAULT_RECONCILE_REQUEUE_INTERVAL: u64 = 60 * 5;

pub(crate) const META_PREFIX: &'static str = "sart.terassyi.net";

#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Endpoint {
    pub host: Option<String>,
    pub port: u32,
    pub timeout: Option<u64>,
}

impl Endpoint {
    pub fn host(mut self, h: &str) -> Self {
        self.host = Some(h.to_string());
        self
    }

    pub fn create(&self) -> Result<SartSpeaker, Error> {
        let timeout = match self.timeout {
            Some(t) => t,
            None => DEFAULT_ENDPOINT_CONNECT_TIMEOUT,
        };
        let host = match &self.host {
            Some(h) => h,
            None => {
                return Err(Error::InvalidEndpoint);
            }
        };
        
        let endpoint = format!("{}:{}", host, self.port);
        Ok(speaker::speaker::new::<SartSpeaker>(&endpoint, timeout))
    }
}

pub(crate) fn finalizer(kind: &str) -> String {
    format!("{}.{}/finalizer", kind, META_PREFIX)
}



pub(crate) fn error_policy<T: Resource<DynamicType = ()>>(resource: Arc<T>, error: &Error, ctx: Arc<Context>) -> Action {
    tracing::warn!("reconcile failed: {:?}", error);
    ctx.metrics.reconcile_failure(resource.as_ref(), error);
    Action::requeue(Duration::from_secs(5 * 60))
}

pub(crate) fn get_node_internal_addr(node: &Node) -> Option<IpAddr> {
    if let Some(status) = &node.status {
        if let Some(addrs) = &status.addresses {
            addrs.iter().find(|addr| addr.type_.eq("InternalIP")).map(|addr| IpAddr::from_str(&addr.address).ok()).flatten()
        } else {
            None
        }
    } else {
        None
    }
}

pub(crate) fn get_match_labels(cb: &ClusterBgp) -> Option<Vec<String>> {
    let mut matched_labels: Vec<String> = vec![];
    if let Some(node_selector) = &cb.spec.policy.node_selector {
        if let Some(labels) = &node_selector.match_labels {
            for (k, v) in labels.iter() {
                let l = format!("{}={}", k, v);
                matched_labels.push(l);
            }
        }
    }
    if matched_labels.is_empty() {
        return None;
    }
    Some(matched_labels)
}
