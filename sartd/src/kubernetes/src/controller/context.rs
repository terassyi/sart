use std::{sync::{Arc, Mutex}, time::Duration};

use chrono::{DateTime, Utc};
use http::{Request, Response};
use hyper::Body;
pub use kube::{
    core::DynamicResourceScope,
    runtime::{
        controller::Action,
        events::{Recorder, Reporter},
    },
    Client, Resource,
};
use prometheus::Registry;
use serde::Serialize;
use tokio::sync::RwLock;

use sartd_trace::error::TraceableError;

use crate::fixture::reconciler::ApiServerVerifier;

use super::metrics::Metrics;

pub trait Ctx {
    fn metrics(&self) -> Arc<Mutex<Metrics>>;
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
    pub metrics: Arc<Mutex<Metrics>>,
}

impl Ctx for Context {
    fn client(&self) -> &Client {
        &self.client
    }

    fn metrics(&self) -> Arc<Mutex<Metrics>> {
        self.metrics.clone()
    }
}

pub struct ContextWith<T: Clone> {
    pub(crate) inner: Context,
    pub component: T,
}

impl<T: Clone> Ctx for ContextWith<T> {
    fn client(&self) -> &Client {
        &self.inner.client
    }

    fn metrics(&self) -> Arc<Mutex<Metrics>> {
        self.inner.metrics.clone()
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
    pub fn to_context(&self, client: Client, interval: u64, metrics: Arc<Mutex<Metrics>>) -> Arc<Context> {
        Arc::new(Context {
            client,
            interval,
            metrics,
            diagnostics: self.diagnostics.clone(),
        })
    }
    pub fn to_context_with<T: Clone>(
        &self,
        client: Client,
        interval: u64,
        component: T,
        metrics: Arc<Mutex<Metrics>>,
    ) -> Arc<ContextWith<T>> {
        Arc::new(ContextWith {
            inner: Context {
                client,
                interval,
                diagnostics: self.diagnostics.clone(),
                metrics,
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
    let metrics = ctx.metrics();
    metrics.lock().unwrap().reconcile_failure(resource.as_ref());
    Action::requeue(Duration::from_secs(10))
}

impl Context {
    pub fn test() -> (Arc<Self>, ApiServerVerifier, Registry) {
        let (mock_service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
        let mock_client = Client::new(mock_service, "default");
        let registry = Registry::default();
        let ctx = Self {
            client: mock_client,
            metrics: Arc::new(Mutex::new(Metrics::default().register(&registry).unwrap())),
            diagnostics: Arc::default(),
            interval: 30,
        };
        (Arc::new(ctx), ApiServerVerifier(handle), registry)
    }
}

impl<T: Clone> ContextWith<T> {
    pub fn test(component: T) -> (Arc<Self>, ApiServerVerifier, Registry) {
        let (mock_service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
        let mock_client = Client::new(mock_service, "default");
        let registry = Registry::default();
        let ctx = Context {
            client: mock_client,
            metrics: Arc::new(Mutex::new(Metrics::default().register(&registry).unwrap())),
            diagnostics: Arc::default(),
            interval: 30,
        };
        let ctx_with = Self {
            inner: ctx,
            component,
        };
        (Arc::new(ctx_with), ApiServerVerifier(handle), registry)
    }
}
