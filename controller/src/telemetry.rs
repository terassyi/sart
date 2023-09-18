use opentelemetry::trace::TraceId;
use tracing_subscriber::{prelude::*, EnvFilter, Registry};

///  Fetch an opentelemetry::trace::TraceId as hex through the full tracing stack
pub fn get_trace_id() -> TraceId {
    use opentelemetry::trace::TraceContextExt as _; // opentelemetry::Context -> opentelemetry::trace::Span
    use tracing_opentelemetry::OpenTelemetrySpanExt as _; // tracing::Span to opentelemetry::Context

    tracing::Span::current()
        .context()
        .span()
        .span_context()
        .trace_id()
}

pub async fn init() {
    // Setup tracing layers
    #[cfg(feature = "telemetry")]
    let telemetry = tracing_opentelemetry::layer().with_tracer(init_tracer().await);
    let logger = tracing_subscriber::fmt::layer().compact();
    let env_filter = EnvFilter::try_from_default_env()
        .or(EnvFilter::try_new("info"))
        .unwrap();

    // Decide on layers
    #[cfg(feature = "telemetry")]
    let collector = Registry::default().with(telemetry).with(logger).with(env_filter);
    #[cfg(not(feature = "telemetry"))]
    let collector = Registry::default().with(logger).with(env_filter);

    // Initialize tracing
    tracing::subscriber::set_global_default(collector).unwrap();
}
