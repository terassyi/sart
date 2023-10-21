use actix_web::{
    get, middleware, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use prometheus::{Encoder, TextEncoder};
use rustls::ServerConfig;

use crate::{
    cert::util::*,
    kubernetes::context::State,
    trace::init::{prepare_tracing, TraceConfig},
};

use super::config::Config;
use super::reconciler;

pub(crate) fn start(config: Config, trace: TraceConfig) {
    let agent = Agent::new(config);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run(agent, trace));
}

#[tracing::instrument(skip_all)]
async fn run(a: Agent, trace_config: TraceConfig) {
    prepare_tracing(trace_config).await;

    // Configure TLS settings
    let cert_chain = load_certificates_from_pem(&a.server_cert).unwrap();
    let private_key = load_private_key_from_file(&a.server_key).unwrap();

    // Initiatilize Kubernetes controller state
    let state = State::default();

    // Start web server
    let server_config = ServerConfig::builder()
        .with_safe_default_cipher_suites()
        .with_safe_default_kx_groups()
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)
        .expect("Bad certificate or key are given");

    let server_state = state.clone();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(Data::new(server_state.clone()))
            .wrap(middleware::Logger::default().exclude("/healthz"))
            .wrap(middleware::Logger::default().exclude("/readyz"))
            .service(index)
            .service(health)
            .service(ready)
            .service(metrics_)
    })
    .bind_rustls_021("0.0.0.0:9443", server_config)
    .unwrap()
    .bind("0.0.0.0:8080")
    .unwrap()
    .shutdown_timeout(5);

    tracing::info!("Start Agent Reconcilers");

    let node_bgp_state = state.clone();
    tokio::spawn(async move {
        reconciler::node_bgp::run(node_bgp_state, a.requeue_interval).await;
    });

    let bgp_peer_state = state.clone();
    tokio::spawn(async move {
        reconciler::bgp_peer::run(bgp_peer_state, a.requeue_interval).await;
    });

    let bgp_advertisement_state = state.clone();
    tokio::spawn(async move {
        reconciler::bgp_advertisement::run(bgp_advertisement_state, a.requeue_interval).await;
    });

    tokio::spawn(async move {
        reconciler::bgp_peer_watcher::run(&a.peer_state_watcher).await;
    });

    server.run().await.unwrap()
}

pub(crate) struct Agent {
    endpoint: String,
    server_cert: String,
    server_key: String,
    requeue_interval: u64,
    peer_state_watcher: String,
}

impl Agent {
    pub fn new(config: Config) -> Self {
        Self {
            endpoint: config.endpoint,
            server_cert: config.tls.cert,
            server_key: config.tls.key,
            requeue_interval: config.requeue_interval,
            peer_state_watcher: config.peer_state_watcher,
        }
    }
}

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
