use std::sync::Arc;

use kube::{Resource, runtime::controller::Action, core::DynamicResourceScope};

use tokio::time::Duration;

use crate::error::Error;
use crate::context::Context;





pub(crate) fn error_policy<T: Resource<DynamicType = ()>>(resource: Arc<T>, error: &Error, ctx: Arc<Context>) -> Action {
    tracing::warn!("reconcile failed: {:?}", error);
    ctx.metrics.reconcile_failure(resource.as_ref(), error);
    Action::requeue(Duration::from_secs(5 * 60))
}
