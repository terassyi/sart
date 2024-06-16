use kube::Resource;
use prometheus::Registry;
use prometheus::{histogram_opts, opts, HistogramVec, IntCounterVec};
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct Metrics {
    pub reconciliations: IntCounterVec,
    pub failures: IntCounterVec,
    pub reconcile_duration: HistogramVec,
}

impl Default for Metrics {
    fn default() -> Self {
        let reconcile_duration = HistogramVec::new(
            histogram_opts!(
                "sart_controller_reconcile_duration_seconds",
                "The duration of reconcile to complete in seconds"
            )
            .buckets(vec![0.01, 0.1, 0.25, 0.5, 1., 5., 15., 60.]),
            &[],
        )
        .unwrap();
        let failures = IntCounterVec::new(
            opts!(
                "sart_controller_reconciliation_errors_total",
                "reconciliation errors",
            ),
            &["resource", "instance", "error"],
        )
        .unwrap();
        let reconciliations = IntCounterVec::new(
            opts!(
                "sart_controller_reconciliation_total",
                "Total count of reconciliations",
            ),
            &["resource", "instance"],
        )
        .unwrap();
        Metrics {
            reconciliations,
            failures,
            reconcile_duration,
        }
    }
}

impl Metrics {
    pub fn register(self, registry: &Registry) -> Result<Self, prometheus::Error> {
        registry.register(Box::new(self.reconciliations.clone()))?;
        registry.register(Box::new(self.failures.clone()))?;
        registry.register(Box::new(self.reconcile_duration.clone()))?;
        Ok(self)
    }

    pub fn reconcile_failure<T: Resource<DynamicType = ()>>(&self, resource: &T) {
        self.failures
            .with_label_values(&[
                &resource.object_ref(&()).kind.unwrap(),
                &resource.object_ref(&()).name.unwrap(),
            ])
            .inc()
    }

    pub fn reconciliation<T: Resource<DynamicType = ()>>(&self, resource: &T) {
        self.reconciliations
            .with_label_values(&[
                &resource.object_ref(&()).kind.unwrap(),
                &resource.object_ref(&()).name.unwrap(),
            ])
            .inc()
    }
}

/// Smart function duration measurer
///
/// Relies on Drop to calculate duration and register the observation in the histogram
pub struct ReconcileMeasurer {
    start: Instant,
    metric: HistogramVec,
}

impl Drop for ReconcileMeasurer {
    fn drop(&mut self) {
        #[allow(clippy::cast_precision_loss)]
        let duration = self.start.elapsed().as_millis() as f64 / 1000.0;
        self.metric.with_label_values(&[]).observe(duration);
    }
}
