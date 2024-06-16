use kube::Resource;
use prometheus::{histogram_opts, opts, HistogramVec, IntCounterVec};
use prometheus::{IntGaugeVec, Registry};
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct Metrics {
    pub reconciliations: IntCounterVec,
    pub failures: IntCounterVec,
    pub reconcile_duration: HistogramVec,
    pub max_blocks: IntGaugeVec,
    pub allocated_blocks: IntGaugeVec,
    pub bgp_advertisements: IntGaugeVec,
    pub bgp_advertisement_status: IntGaugeVec,
    pub bgp_advertisement_backoff: IntCounterVec,
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
                "Total count of reconciliation errors",
            ),
            &["resource", "instance"],
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
        let max_blocks = IntGaugeVec::new(
            opts!(
                "sart_controller_max_blocks",
                "The number of maximum allocatable address blocks"
            ),
            &["pool", "type"],
        )
        .unwrap();
        let allocated_blocks = IntGaugeVec::new(
            opts!(
                "sart_controller_allocated_blocks",
                "The number of allocated address blocks"
            ),
            &["pool", "type"],
        )
        .unwrap();
        let bgp_advertisements = IntGaugeVec::new(
            opts!(
                "sart_controller_bgp_advertisements",
                "The number of BGP Advertisement"
            ),
            &["type"],
        )
        .unwrap();
        let bgp_advertisement_status = IntGaugeVec::new(
            opts!(
                "sart_controller_bgp_advertisement_status",
                "BGP Advertisement status"
            ),
            &["name", "status"],
        )
        .unwrap();
        let bgp_advertisement_backoff = IntCounterVec::new(
            opts!(
                "sart_controller_bgp_advertisement_backoff_count",
                "The number of back off count of BGP Advertisement "
            ),
            &["name"],
        )
        .unwrap();

        Metrics {
            reconciliations,
            failures,
            reconcile_duration,
            max_blocks,
            allocated_blocks,
            bgp_advertisements,
            bgp_advertisement_status,
            bgp_advertisement_backoff,
        }
    }
}

impl Metrics {
    pub fn register(self, registry: &Registry) -> Result<Self, prometheus::Error> {
        registry.register(Box::new(self.reconciliations.clone()))?;
        registry.register(Box::new(self.failures.clone()))?;
        registry.register(Box::new(self.reconcile_duration.clone()))?;
        registry.register(Box::new(self.max_blocks.clone()))?;
        registry.register(Box::new(self.allocated_blocks.clone()))?;
        registry.register(Box::new(self.bgp_advertisements.clone()))?;
        registry.register(Box::new(self.bgp_advertisement_status.clone()))?;
        registry.register(Box::new(self.bgp_advertisement_backoff.clone()))?;
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

    pub fn max_blocks(&self, pool: &str, r#type: &str, val: i64) {
        self.max_blocks.with_label_values(&[pool, r#type]).set(val)
    }

    pub fn allocated_blocks_inc(&self, pool: &str, r#type: &str) {
        self.allocated_blocks
            .with_label_values(&[pool, r#type])
            .inc()
    }

    pub fn allocated_blocks_dec(&self, pool: &str, r#type: &str) {
        self.allocated_blocks
            .with_label_values(&[pool, r#type])
            .dec()
    }

    pub fn bgp_advertisements_inc(&self, r#type: &str) {
        self.bgp_advertisements.with_label_values(&[r#type]).inc()
    }

    pub fn bgp_advertisements_dec(&self, r#type: &str) {
        self.bgp_advertisements.with_label_values(&[r#type]).dec()
    }

    pub fn bgp_advertisements_set(&self, r#type: &str, val: i64) {
        self.bgp_advertisements
            .with_label_values(&[r#type])
            .set(val)
    }

    pub fn bgp_advertisement_status_set(&self, name: &str, status: &str, val: i64) {
        self.bgp_advertisement_status
            .with_label_values(&[name, status])
            .set(val)
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
