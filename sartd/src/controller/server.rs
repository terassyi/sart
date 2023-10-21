use actix_web::{
    get, middleware, post,
    web::{self, Data},
    App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use kube::core::admission::AdmissionReview;
use prometheus::{Encoder, TextEncoder};
use rustls::ServerConfig;

use super::reconciler;
use crate::{
    cert::util::*,
    controller::webhook,
    kubernetes::{context::State, crd::{bgp_peer::BGPPeer, bgp_advertisement::BGPAdvertisement}},
    trace::init::{prepare_tracing, TraceConfig},
};

use super::config::Config;

pub(crate) fn start(config: Config, trace: TraceConfig) {
    let agent = Controller::new(config);

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(run(agent, trace));
}

#[tracing::instrument(skip_all)]
async fn run(c: Controller, trace_config: TraceConfig) {
    prepare_tracing(trace_config).await;

    // Configure TLS settings
    let cert_chain = load_certificates_from_pem(&c.server_cert).unwrap();
    let private_key = load_private_key_from_file(&c.server_key).unwrap();

    // Initiatilize Kubernetes controller state
    let state = State::new("controller");

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
            .service(bgp_peer_validating_webhook)
            .service(bgp_peer_mutating_webhook)
            .service(bgp_advertisement_validating_webhook)
    })
    .bind_rustls_021("0.0.0.0:8443", server_config)
    .unwrap()
    .bind("0.0.0.0:8080")
    .unwrap()
    .shutdown_timeout(5);

    // Start reconcilers
    tracing::info!("Start ClusterBGP reconciler");
    let cluster_bgp_state = state.clone();
    tokio::spawn(async move {
        reconciler::cluster_bgp::run(cluster_bgp_state, c.requeue_interval).await;
    });

    tracing::info!("Start AddressPool reconciler");
    let address_pool_state = state.clone();
    tokio::spawn(async move {
        reconciler::address_pool::run(address_pool_state, c.requeue_interval).await;
    });

    tracing::info!("Start Endpointslice watcher");
    let endpointslice_state = state.clone();
    tokio::spawn(async move {
        reconciler::service_watcher::run(endpointslice_state, c.requeue_interval).await;
    });

    server.run().await.unwrap()
}

pub(crate) struct Controller {
    endpoint: String,
    server_cert: String,
    server_key: String,
    requeue_interval: u64,
}

impl Controller {
    pub fn new(config: Config) -> Self {
        Self {
            endpoint: config.endpoint,
            server_cert: config.tls.cert,
            server_key: config.tls.key,
            requeue_interval: config.requeue_interval,
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

#[post("/validate-sart-terassyi-net-v1alpha2-bgppeer")]
async fn bgp_peer_validating_webhook(
    req: HttpRequest,
    body: web::Json<AdmissionReview<BGPPeer>>,
) -> impl Responder {
    webhook::bgp_peer::handle_validation(req, body).await
}

#[post("/mutate-sart-terassyi-net-v1alpha2-bgppeer")]
async fn bgp_peer_mutating_webhook(
    req: HttpRequest,
    body: web::Json<AdmissionReview<BGPPeer>>,
) -> impl Responder {
    webhook::bgp_peer::handle_mutation(req, body).await
}

#[post("/validate-sart-terassyi-net-v1alpha2-bgpadvertisement")]
async fn bgp_advertisement_validating_webhook(
    req: HttpRequest,
    body: web::Json<AdmissionReview<BGPAdvertisement>>,
) -> impl Responder {
    webhook::bgp_advertisement::handle_validation(req, body).await
}
