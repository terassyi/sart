use std::sync::Arc;
use std::{net::IpAddr, sync::Mutex};

use actix_web::{
    get, middleware, web::Data, App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use k8s_openapi::api::core::v1::Node;
use kube::{Api, Client};
use prometheus::{Encoder, TextEncoder};
use rustls::ServerConfig;
use sartd_cert::util::{load_certificates_from_pem, load_private_key_from_file};
use sartd_ipam::manager::AllocatorSet;
use sartd_trace::init::{prepare_tracing, TraceConfig};
use tokio::sync::mpsc::unbounded_channel;

use crate::agent::{
    cni::{
        self,
        server::{CNIServer, CNI_ROUTE_TABLE_ID},
    },
    context::State,
    metrics::Metrics,
    reconciler::address_block::PodAllocator,
};
use crate::config::Mode;
use crate::crd::address_block::AddressBlock;

use super::config::Config;
use super::reconciler;

pub fn start(config: Config, trace: TraceConfig) {
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

    // Initialize Kubernetes controller state
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
            .service(index)
            .service(health)
            .service(ready)
            .service(metrics_)
            .wrap(
                middleware::Logger::default()
                    .exclude("/healthz")
                    .exclude("/readyz"),
            )
    })
    .bind_rustls_021(format!("0.0.0.0:{}", a.server_https_port), server_config)
    .unwrap()
    .bind(format!("0.0.0.0:{}", a.server_http_port))
    .unwrap()
    .shutdown_timeout(5);

    tracing::info!(
        http_port = a.server_http_port,
        https_port = a.server_https_port,
        "Agent server is running."
    );

    tracing::info!("Start Agent Reconcilers");

    let metrics = Arc::new(Mutex::new(
        Metrics::default().register(&state.registry).unwrap(),
    ));

    let node_bgp_state = state.clone();
    let nb_metrics = metrics.clone();
    tokio::spawn(async move {
        reconciler::node_bgp::run(node_bgp_state, a.requeue_interval, nb_metrics).await;
    });

    let bgp_peer_state = state.clone();
    let bp_metrics = metrics.clone();
    tokio::spawn(async move {
        reconciler::bgp_peer::run(bgp_peer_state, a.requeue_interval, bp_metrics).await;
    });

    let bgp_advertisement_state = state.clone();
    let ba_metrics = metrics.clone();
    tokio::spawn(async move {
        reconciler::bgp_advertisement::run(bgp_advertisement_state, a.requeue_interval, ba_metrics)
            .await;
    });

    let pw_metrics = metrics.clone();
    tokio::spawn(async move {
        reconciler::bgp_peer_watcher::run(&a.peer_state_watcher, pw_metrics).await;
    });

    if a.mode.eq(&Mode::CNI) || a.mode.eq(&Mode::Dual) {
        // Pod address allocators
        let allocator_set = Arc::new(AllocatorSet::new());
        let (sender, receiver) = unbounded_channel::<AddressBlock>();
        let pod_allocator = Arc::new(PodAllocator {
            allocator: allocator_set.clone(),
            notifier: sender,
        });

        let address_block_state = state.clone();
        let ab_metrics = metrics.clone();
        let ab_pod_allocator = pod_allocator.clone();
        tokio::spawn(async move {
            reconciler::address_block::run(
                address_block_state,
                a.requeue_interval,
                ab_pod_allocator,
                ab_metrics,
            )
            .await;
        });

        let node_name = std::env::var("HOSTNAME").unwrap();
        // get node internal ip
        let cni_metrics = metrics.clone();
        let node_addr = get_node_addr(&node_name).await;
        let kube_client = Client::try_default().await.unwrap();
        let cni_server = CNIServer::new(
            kube_client.clone(),
            allocator_set.clone(),
            node_name,
            node_addr,
            CNI_ROUTE_TABLE_ID,
            receiver,
            cni_metrics,
        );

        let cni_endpoint = a.cni_endpoint.expect("cni endpoint must be given");

        tracing::info!("Start CNI server");
        tokio::spawn(async move {
            cni::server::run(&cni_endpoint, cni_server).await;
        });
    }

    server.run().await.unwrap()
}

pub struct Agent {
    server_http_port: u32,
    server_https_port: u32,
    endpoint: String,
    server_cert: String,
    server_key: String,
    requeue_interval: u64,
    peer_state_watcher: String,
    cni_endpoint: Option<String>,
    mode: Mode,
}

impl Agent {
    pub fn new(config: Config) -> Self {
        Self {
            server_http_port: config.http_port,
            server_https_port: config.https_port,
            endpoint: config.endpoint,
            server_cert: config.tls.cert,
            server_key: config.tls.key,
            requeue_interval: config.requeue_interval,
            peer_state_watcher: config.peer_state_watcher,
            mode: config.mode,
            cni_endpoint: config.cni_endpoint.clone(),
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

// This function can panic
async fn get_node_addr(name: &str) -> IpAddr {
    let client = Client::try_default().await.unwrap();
    let node_api = Api::<Node>::all(client);
    let node = node_api.get(name).await.unwrap();
    for addr in node.status.unwrap().addresses.unwrap().iter() {
        if addr.type_.eq("InternalIP") {
            return addr.address.parse::<IpAddr>().unwrap();
        }
    }
    panic!("Failed to get Node internal IP");
}
