use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use kube::{Client, runtime::{events::{Reporter, Recorder}, controller::Action}, Resource, core::DynamicResourceScope};
use serde::Serialize;
use tokio::sync::RwLock;

use crate::trace::{metrics::Metrics, error::TraceableError};

// Context for our reconciler
#[derive(Clone)]
pub(crate) struct Context {
    // Kubernetes client
    pub client: Client,
    // Reconcile interval
    pub interval: u64,
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
    pub fn to_context(&self, client: Client, interval: u64) -> Arc<Context> {
        Arc::new(Context {
            client,
            interval,
            metrics: Metrics::default().register(&self.registry).unwrap(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}

impl State {
    pub fn new(component: &str) -> State {
        State {
            diagnostics: Arc::new(RwLock::new(Diagnostics::new(component.to_string()))),
            registry: prometheus::Registry::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Diagnostics {
    #[serde(deserialize_with = "from_ts")]
    pub last_event: DateTime<Utc>,
    #[serde(skip)]
    pub reporter: Reporter,
}

impl Diagnostics {
	pub(crate) fn new(component: String) -> Self {
        Self {
            last_event: Utc::now(),
            reporter: component.into(),
		}
	}
}
impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            last_event: Utc::now(),
            reporter: "sart".into(),
        }
    }
}
impl Diagnostics {
    fn recorder<T: Resource<DynamicType = (), Scope = DynamicResourceScope>>(&self, client: Client, res: T) -> Recorder {
        Recorder::new(client, self.reporter.clone(), res.object_ref(&()))
    }
}

pub(crate) fn error_policy<T: Resource<DynamicType = ()>, E: TraceableError>(resource: Arc<T>, error: &E, ctx: Arc<Context>) -> Action {
    tracing::warn!("reconcile failed: {:?}", error);
    ctx.metrics.reconcile_failure(resource.as_ref(), error);
    Action::requeue(Duration::from_secs(5 * 60))
}
