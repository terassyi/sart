use kube::Resource;
use kube::core::DynamicResourceScope;
use prometheus::Registry;
use prometheus::{histogram_opts, opts, HistogramVec, IntCounter, IntCounterVec};

use super::error::Error;


#[derive(Debug, Clone)]
pub(crate) struct Metrics {
	pub reconciliations: IntCounter,
    pub failures: IntCounterVec,
    pub reconcile_duration: HistogramVec,
}

impl Default for Metrics {
	fn default() -> Self {
        let reconcile_duration = HistogramVec::new(
            histogram_opts!(
                "doc_controller_reconcile_duration_seconds",
                "The duration of reconcile to complete in seconds"
            )
            .buckets(vec![0.01, 0.1, 0.25, 0.5, 1., 5., 15., 60.]),
            &[],
        )
        .unwrap();
        let failures = IntCounterVec::new(
            opts!(
                "doc_controller_reconciliation_errors_total",
                "reconciliation errors",
            ),
            &["instance", "error"],
        )
        .unwrap();
        let reconciliations =
            IntCounter::new("doc_controller_reconciliations_total", "reconciliations").unwrap();
        Metrics {
            reconciliations,
            failures,
            reconcile_duration,
        }
	}
}

impl Metrics {
	pub fn register(self, registry: &Registry) -> Result<Self, prometheus::Error> {
        Ok(self)
    }

	pub fn reconcile_failure<T: Resource<DynamicType = ()>>(&self, resource: &T, e: &Error) {
        self.failures
            .with_label_values(&[&resource.object_ref(&()).name.unwrap(), e.metric_label().as_ref()])
            .inc()
    }
}
