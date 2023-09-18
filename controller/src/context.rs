use std::sync::Arc;

use chrono::{DateTime, Utc};
use kube::{Client, runtime::events::{Reporter, Recorder}, Resource, core::DynamicResourceScope};
use serde::Serialize;
use tokio::sync::RwLock;

use crate::metrics::Metrics;


// Context for our reconciler
#[derive(Clone)]
pub(crate) struct Context {
    // Kubernetes client
    pub client: Client,
    // Diagnostics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    // Prometheus metrics
    pub metrics: Metrics,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct State {
	pub diagnostics: Arc<RwLock<Diagnostics>>,
	pub registry: prometheus::Registry,
}

impl State {
    /// Metrics getter
    pub fn metrics(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }

    /// State getter
    pub async fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.read().await.clone()
    }

    // Create a Controller Context that can update State
    pub fn to_context(&self, client: Client) -> Arc<Context> {
        Arc::new(Context {
            client,
            metrics: Metrics::default().register(&self.registry).unwrap(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostics {
    #[serde(deserialize_with = "from_ts")]
    pub last_event: DateTime<Utc>,
    #[serde(skip)]
    pub reporter: Reporter,
}
impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            last_event: Utc::now(),
            reporter: "sart-controller".into(),
        }
    }
}
impl Diagnostics {
    fn recorder<T: Resource<DynamicType = (), Scope = DynamicResourceScope>>(&self, client: Client, res: T) -> Recorder {
        Recorder::new(client, self.reporter.clone(), res.object_ref(&()))
    }
}
