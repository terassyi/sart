use opentelemetry::trace::TraceId;
use rand::Rng;
use tracing_subscriber::{prelude::*, Registry};

///  Fetch an opentelemetry::trace::TraceId as hex through the full tracing stack
pub fn get_trace_id() -> TraceId {
    let mut rng = rand::thread_rng();
    let val: u128 = rng.gen();
    TraceId::from(val)
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
