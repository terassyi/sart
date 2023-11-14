use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
pub use kube::{
    core::{DynamicResourceScope, ObjectMeta},
    runtime::{
        controller::Action,
        events::{Recorder, Reporter},
    },
    Client, Resource,
};
use serde::Serialize;
use tokio::sync::RwLock;

use sartd_trace::{error::TraceableError, metrics::Metrics};

pub trait Ctx {
    fn metrics(&self) -> &Metrics;
    fn client(&self) -> &Client;
}

// Context for our reconciler
#[derive(Clone)]
pub struct Context {
    // Kubernetes client
    pub client: Client,
    // Reconcile interval
    pub interval: u64,
    // Diagnostics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    // Prometheus metrics
    pub metrics: Metrics,
}

impl Ctx for Context {
    fn client(&self) -> &Client {
        &self.client
    }

    fn metrics(&self) -> &Metrics {
        &self.metrics
    }
}

pub struct ContextWith<T: Clone> {
    inner: Context,
    pub component: T,
}

impl<T: Clone> Ctx for ContextWith<T> {
    fn client(&self) -> &Client {
        &self.inner.client
    }

    fn metrics(&self) -> &Metrics {
        &self.inner.metrics
    }
}

#[derive(Debug, Clone, Default)]
pub struct State {
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    pub registry: prometheus::Registry,
}

impl State {
    pub fn new(component: &str) -> State {
        State {
            diagnostics: Arc::new(RwLock::new(Diagnostics::new(component.to_string()))),
            registry: prometheus::Registry::default(),
        }
    }
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
    pub fn to_context_with<T: Clone>(
        &self,
        client: Client,
        interval: u64,
        component: T,
    ) -> Arc<ContextWith<T>> {
        Arc::new(ContextWith {
            inner: Context {
                client,
                interval,
                diagnostics: self.diagnostics.clone(),
                metrics: Metrics::default().register(&self.registry).unwrap(),
            },
            component,
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

impl Diagnostics {
    pub fn new(component: String) -> Self {
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
    fn recorder<T: Resource<DynamicType = (), Scope = DynamicResourceScope>>(
        &self,
        client: Client,
        res: T,
    ) -> Recorder {
        Recorder::new(client, self.reporter.clone(), res.object_ref(&()))
    }
}

#[tracing::instrument(skip_all)]
pub fn error_policy<T: Resource<DynamicType = ()>, E: TraceableError, C: Ctx>(
    resource: Arc<T>,
    error: &E,
    ctx: Arc<C>,
) -> Action {
    tracing::warn!("reconcile failed: {:?}", error);
    ctx.metrics().reconcile_failure(resource.as_ref(), error);
    Action::requeue(Duration::from_secs(10))
}
