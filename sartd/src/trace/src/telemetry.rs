use opentelemetry::trace::TraceId;
use tracing_subscriber::{prelude::*, Registry};

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

pub async fn init(level: tracing::Level) {
    // Setup tracing layers
    #[cfg(feature = "telemetry")]
    let telemetry = tracing_opentelemetry::layer().with_tracer(init_tracer().await);
    let logger = tracing_subscriber::fmt::layer().compact();

    // Decide on layers
    #[cfg(feature = "telemetry")]
    let collector = Registry::default()
        .with(telemetry)
        .with(logger)
        .with(tracing_subscriber::filter::LevelFilter::from_level(level));
    #[cfg(not(feature = "telemetry"))]
    let collector = Registry::default()
        .with(logger)
        .with(tracing_subscriber::filter::LevelFilter::from_level(level));

    // Initialize tracing
    tracing::subscriber::set_global_default(collector).unwrap();
}
