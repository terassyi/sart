use std::sync::Arc;

use kube::{Resource, runtime::controller::Action};

use tokio::time::Duration;

use crate::error::Error;
use crate::context::Context;


pub(crate) const DEFAULT_RECONCILE_REQUEUE_INTERVAL: u64 = 60 * 5;

pub(crate) const META_PREFIX: &'static str = "sart.terassyi.net";

pub(crate) fn finalizer(kind: &str) -> String {
    format!("{}.{}/finalizer", kind, META_PREFIX)
}



pub(crate) fn error_policy<T: Resource<DynamicType = ()>>(resource: Arc<T>, error: &Error, ctx: Arc<Context>) -> Action {
    tracing::warn!("reconcile failed: {:?}", error);
    ctx.metrics.reconcile_failure(resource.as_ref(), error);
    Action::requeue(Duration::from_secs(5 * 60))
}
