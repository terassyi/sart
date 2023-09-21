pub(crate) mod reconcilers;
pub(crate) mod context;
pub(crate) mod error;
pub(crate) mod metrics;
pub(crate) mod telemetry;
pub(crate) mod bgp;
pub(crate) mod proto;
pub(crate) mod speaker;



use actix_web::{HttpServer, App, web::Data, middleware, get, HttpRequest, Responder, HttpResponse};
use k8s_openapi::api::core::v1::LoadBalancerIngress;
use prometheus::{TextEncoder, Encoder};

use clap::{Parser, ValueEnum};

use crate::{context::State, reconcilers::common::DEFAULT_RECONCILE_REQUEUE_INTERVAL};


#[get("/healthz")]
async fn health(_: HttpRequest) -> impl Responder {
    HttpResponse::Ok().json("healthy")
}

#[get("/readyz")]
async fn ready(_: HttpRequest) -> impl Responder {
	HttpResponse::Ok().json("ready")
}

#[get("/metrics")]
async fn metrics_(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let metrics = c.metrics();
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    encoder.encode(&metrics, &mut buffer).unwrap();
    HttpResponse::Ok().body(buffer)
}

#[get("/")]
async fn index(c: Data<State>, _req: HttpRequest) -> impl Responder {
    let d = c.diagnostics().await;
    HttpResponse::Ok().json(&d)
}


#[derive(Parser)]
struct Args {
    /// Log level for controller
    #[arg(short, long)]
    #[clap(value_enum, default_value_t=LogLevel::Info)]
    log: LogLevel,
}

#[derive(ValueEnum, Clone, Debug)]
enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl From<LogLevel> for tracing::Level {
    fn from(value: LogLevel) -> Self {
        match value {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let level = tracing::Level::from(args.log);

    telemetry::init(level).await;

    // Initiatilize Kubernetes controller state
    let state = State::default();

    let requeue_interval = DEFAULT_RECONCILE_REQUEUE_INTERVAL;

    // Preparer reconcilers for each resource
    let cluster_bgp_reconciler= reconcilers::clusterbgp::run(state.clone(), requeue_interval);
    let bgp_peer_reconciler = reconcilers::bgppeer::run(state.clone(), requeue_interval);
    let address_pool_reconciler = reconcilers::addresspool::run(state.clone(), requeue_interval);
    let bgp_advertisement_reconciler = reconcilers::bgpadvertisement::run(state.clone(), requeue_interval);


    // Start web server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(state.clone()))
            .wrap(middleware::Logger::default().exclude("/healthz"))
            .wrap(middleware::Logger::default().exclude("/readyz"))
            .service(index)
            .service(health)
            .service(ready)
            .service(metrics_)
    })
    .bind("0.0.0.0:8080")?
    .shutdown_timeout(5);

    // Both runtimes implements graceful shutdown, so poll until both are done
    tracing::info!("Start server and controller");
    tokio::join!(
        server.run(),
        cluster_bgp_reconciler,
        bgp_peer_reconciler, 
        address_pool_reconciler,
        bgp_advertisement_reconciler,
    ).0?;
    Ok(())
}
