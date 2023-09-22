pub(crate) mod reconcilers;
pub(crate) mod context;
pub(crate) mod error;
pub(crate) mod metrics;
pub(crate) mod telemetry;
pub(crate) mod bgp;
pub(crate) mod proto;
pub(crate) mod speaker;
pub(crate) mod webhook;



use std::fs::File;
use std::io::BufReader;

use actix_web::{web, post};
use actix_web::{HttpServer, App, web::Data, middleware, get, HttpRequest, Responder, HttpResponse};

use kube::core::DynamicObject;
use kube::core::admission::AdmissionReview;
use prometheus::{TextEncoder, Encoder};


use clap::{Parser, ValueEnum};
use rustls::{Certificate, PrivateKey, ServerConfig};


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

#[post("/validate-sart-terassyi-net-v1alpha2-bgppeer")]
async fn bgppeer_validation_webhook(
    req: HttpRequest,
    body: web::Json<AdmissionReview<DynamicObject>>,
) -> impl Responder {
    webhook::bgppeer::handle(req, body).await
}


#[derive(Parser)]
struct Args {
    /// Log level for controller
    #[arg(short, long)]
    #[clap(value_enum, default_value_t=LogLevel::Info)]
    log: LogLevel,

    /// Path to the server certificate file
    #[arg(long)]
    #[clap(default_value = "/etc/controller/cert/tls.crt")]
    server_cert: String,

    /// Path to the server key file
    #[arg(long)]
    #[clap(default_value = "/etc/controller/cert/tls.key")]
    server_key: String,
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

    let server_cert = args.server_cert;
    let server_key = args.server_key;

    telemetry::init(level).await;

    // Initiatilize Kubernetes controller state
    let state = State::default();

    let requeue_interval = DEFAULT_RECONCILE_REQUEUE_INTERVAL;

    // Preparer reconcilers for each resource
    let cluster_bgp_reconciler= reconcilers::clusterbgp::run(state.clone(), requeue_interval);
    let bgp_peer_reconciler = reconcilers::bgppeer::run(state.clone(), requeue_interval);
    let address_pool_reconciler = reconcilers::addresspool::run(state.clone(), requeue_interval);
    let bgp_advertisement_reconciler = reconcilers::bgpadvertisement::run(state.clone(), requeue_interval);


    // Configure TLS settings
    let cert_chain = load_certificates_from_pem(&server_cert)?;
    let private_key = load_private_key_from_file(&server_key).unwrap();

    let server_config= ServerConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kx_groups()
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)
        .expect("Bad certificate or key are given");

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
            .service(bgppeer_validation_webhook)
    })
    .bind_rustls_021("0.0.0.0:9443", server_config)?
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

fn load_certificates_from_pem(path: &str) -> std::io::Result<Vec<Certificate>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let certs = rustls_pemfile::certs(&mut reader)?;

    Ok(certs.into_iter().map(Certificate).collect())
}

fn load_private_key_from_file(path: &str) -> Result<PrivateKey, Box<dyn std::error::Error>> {
    let file = File::open(&path)?;
    let mut reader = BufReader::new(file);

    let mut keys = rustls_pemfile::pkcs8_private_keys(&mut reader)?;

    match keys.len() {
        0 => Err(format!("No PKC8-encoded private key found in {path}").into()),
        1 => Ok(PrivateKey(keys.remove(0))),
        _ => Err(format!("More than one PKC8-encoded private key found in {path}").into()),
    }
}
