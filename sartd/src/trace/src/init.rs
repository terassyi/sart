use std::str::FromStr;

use tracing_subscriber::{prelude::*, Registry};

#[derive(Debug)]
pub struct TraceConfig {
    pub level: String,
    pub format: String,
    pub file: Option<String>,
    pub _metrics_endpoint: Option<String>,
}

pub async fn prepare_tracing(conf: TraceConfig) {
    #[cfg(feature = "telemetry")]
    let span = SpanBuilder::default().with_kind(opentelemetry::trace::SpanKind::Internal);
    // Configure otel exporter.

    #[cfg(not(feature = "telemetry"))]
    if conf.format == "json" {
        if let Some(path) = conf.file {
            let file = std::fs::File::create(path).unwrap();
            Registry::default()
                .with(tracing_subscriber::fmt::Layer::new().with_writer(file))
                .with(tracing_subscriber::fmt::Layer::new().with_ansi(true).json())
                .with(tracing_subscriber::filter::LevelFilter::from_str(&conf.level).unwrap())
                .init();
        } else {
            Registry::default()
                .with(tracing_subscriber::fmt::Layer::new().with_ansi(true).json())
                .with(tracing_subscriber::filter::LevelFilter::from_str(&conf.level).unwrap())
                .init();
        }
    } else if let Some(path) = conf.file {
        let file = std::fs::File::create(path).unwrap();
        Registry::default()
            .with(tracing_subscriber::fmt::Layer::new().with_writer(file))
            .with(tracing_subscriber::fmt::Layer::new().with_ansi(true))
            .with(tracing_subscriber::filter::LevelFilter::from_str(&conf.level).unwrap())
            .init();
    } else {
        Registry::default()
            .with(tracing_subscriber::fmt::Layer::new().with_ansi(true))
            .with(tracing_subscriber::filter::LevelFilter::from_str(&conf.level).unwrap())
            .init();
    }
}
