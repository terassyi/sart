use std::str::FromStr;

use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};

#[derive(Debug)]
pub(crate) struct TraceConfig {
    pub level: String,
    pub format: String,
    pub file: Option<String>,
    pub _metrics_endpoint: Option<String>,
}

pub(crate) fn prepare_tracing(conf: TraceConfig) {
    // Configure otel exporter.
    if conf.format == "json" {
        if let Some(path) = conf.file {
            let file = std::fs::File::create(path).unwrap();
            tracing_subscriber::Registry::default()
                .with(tracing_subscriber::fmt::Layer::new().with_writer(file))
                .with(tracing_subscriber::fmt::Layer::new().with_ansi(true).json())
                .with(tracing_subscriber::filter::LevelFilter::from_str(&conf.level).unwrap())
                .init();
        } else {
            tracing_subscriber::Registry::default()
                .with(tracing_subscriber::fmt::Layer::new().with_ansi(true).json())
                .with(tracing_subscriber::filter::LevelFilter::from_str(&conf.level).unwrap())
                .init();
        }
    } else if let Some(path) = conf.file {
        let file = std::fs::File::create(path).unwrap();
        tracing_subscriber::Registry::default()
            .with(tracing_subscriber::fmt::Layer::new().with_writer(file))
            .with(tracing_subscriber::fmt::Layer::new().with_ansi(true))
            .with(tracing_subscriber::filter::LevelFilter::from_str(&conf.level).unwrap())
            .init();
    } else {
        tracing_subscriber::Registry::default()
            .with(tracing_subscriber::fmt::Layer::new().with_ansi(true))
            .with(tracing_subscriber::filter::LevelFilter::from_str(&conf.level).unwrap())
            .init();
    }
}
