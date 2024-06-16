use kube::Resource;
use prometheus::{histogram_opts, opts, HistogramVec, IntCounterVec};
use prometheus::{IntGaugeVec, Registry};
use tokio::time::Instant;

#[derive(Debug, Clone)]
pub struct Metrics {
    pub reconciliations: IntCounterVec,
    pub failures: IntCounterVec,
    pub reconcile_duration: HistogramVec,
    pub cni_call: IntCounterVec,
    pub cni_errors: IntCounterVec,
    pub bgp_peer_status: IntGaugeVec,
    pub node_bgp_status: IntGaugeVec,
    pub node_bgp_backoff_count: IntCounterVec,
}

impl Default for Metrics {
    fn default() -> Self {
        let reconcile_duration = HistogramVec::new(
            histogram_opts!(
                "sart_agent_reconcile_duration_seconds",
                "The duration of reconcile to complete in seconds"
            )
            .buckets(vec![0.01, 0.1, 0.25, 0.5, 1., 5., 15., 60.]),
            &[],
        )
        .unwrap();
        let failures = IntCounterVec::new(
            opts!(
                "sart_agent_reconciliation_errors_total",
                "Total count of reconciliation errors",
            ),
            &["resource", "instance"],
        )
        .unwrap();
        let reconciliations = IntCounterVec::new(
            opts!(
                "sart_agent_reconciliation_total",
                "Total count of reconciliations",
            ),
            &["resource", "instance"],
        )
        .unwrap();
        let cni_call = IntCounterVec::new(
            opts!("sart_agent_cni_call_total", "Total count of CNI call",),
            &["method"],
        )
        .unwrap();
        let cni_errors = IntCounterVec::new(
            opts!(
                "sart_agent_cni_call_errors_total",
                "Total count of CNI call error"
            ),
            &["method", "code"],
        )
        .unwrap();
        let bgp_peer_status = IntGaugeVec::new(
            opts!("sart_agent_bgp_peer_status", "BGP peer status"),
            &["peer", "status"],
        )
        .unwrap();
        let node_bgp_status = IntGaugeVec::new(
            opts!("sart_agent_node_bgp_status", "Node BGP status"),
            &["name", "status"],
        )
        .unwrap();
        let node_bgp_backoff_count = IntCounterVec::new(
            opts!(
                "sart_agent_node_bgp_backoff_count_total",
                "NodeBGP backoff count"
            ),
            &["name"],
        )
        .unwrap();
        Metrics {
            reconciliations,
            failures,
            reconcile_duration,
            cni_call,
            cni_errors,
            bgp_peer_status,
            node_bgp_status,
            node_bgp_backoff_count,
        }
    }
}

impl Metrics {
    pub fn register(self, registry: &Registry) -> Result<Self, prometheus::Error> {
        registry.register(Box::new(self.reconciliations.clone()))?;
        registry.register(Box::new(self.failures.clone()))?;
        registry.register(Box::new(self.reconcile_duration.clone()))?;
        registry.register(Box::new(self.cni_call.clone()))?;
        registry.register(Box::new(self.cni_errors.clone()))?;
        registry.register(Box::new(self.bgp_peer_status.clone()))?;
        registry.register(Box::new(self.node_bgp_status.clone()))?;
        registry.register(Box::new(self.node_bgp_backoff_count.clone()))?;
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

    pub fn cni_call(&self, method: &str) {
        self.cni_call.with_label_values(&[method]).inc()
    }

    pub fn cni_errors(&self, method: &str, code: &str) {
        self.cni_errors.with_label_values(&[method, code]).inc()
    }

    pub fn bgp_peer_status_up(&self, peer: &str, status: &str) {
        self.bgp_peer_status
            .with_label_values(&[peer, status])
            .set(1)
    }

    pub fn bgp_peer_status_down(&self, peer: &str, status: &str) {
        self.bgp_peer_status
            .with_label_values(&[peer, status])
            .set(0)
    }

    pub fn node_bgp_status_up(&self, name: &str, status: &str) {
        self.node_bgp_status
            .with_label_values(&[name, status])
            .set(1)
    }

    pub fn node_bgp_status_down(&self, name: &str, status: &str) {
        self.node_bgp_status
            .with_label_values(&[name, status])
            .set(0)
    }

    pub fn node_bgp_backoff_count(&self, name: &str) {
        self.node_bgp_backoff_count.with_label_values(&[name]).inc()
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
