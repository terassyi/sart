use std::{collections::HashMap, str::FromStr, sync::{Arc, Mutex}};

use actix_web::{
    get, middleware, post,
    web::{self, Data},
    App, HttpRequest, HttpResponse, HttpServer, Responder,
};
use ipnet::IpNet;
use k8s_openapi::api::core::v1::Service;
use kube::{api::ListParams, core::admission::AdmissionReview, Api, Client, ResourceExt};
use prometheus::{Encoder, TextEncoder};
use rustls::ServerConfig;
use sartd_cert::util::{load_certificates_from_pem, load_private_key_from_file};
use sartd_ipam::manager::{AllocatorSet, BlockAllocator, Pool};
use sartd_trace::init::{prepare_tracing, TraceConfig};

use crate::{
    config::Mode,
    controller::{
        context::State, metrics::Metrics, reconciler::address_block::ControllerAddressBlockContext,
    },
    crd::{
        address_block::AddressBlock, address_pool::AddressPool,
        bgp_advertisement::BGPAdvertisement, bgp_peer::BGPPeer,
    },
};

use super::{error::Error, reconciler, webhook};

use super::config::Config;

pub fn start(config: Config, trace: TraceConfig) {
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

    // Initialize Kubernetes controller state
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
            .service(index)
            .service(health)
            .service(ready)
            .service(metrics_)
            .service(bgp_peer_validating_webhook)
            .service(bgp_peer_mutating_webhook)
            .service(bgp_advertisement_validating_webhook)
            .service(address_pool_validating_webhook)
            .service(address_block_mutating_webhook)
            .service(service_mutating_webhook)
            .wrap(
                middleware::Logger::default()
                    .exclude("/healthz")
                    .exclude("/readyz"),
            )
    })
    .bind_rustls_021(format!("0.0.0.0:{}", c.server_https_port), server_config)
    .unwrap()
    .bind(format!("0.0.0.0:{}", c.server_port))
    .unwrap()
    .shutdown_timeout(5);

    tracing::info!(
        http_port = c.server_port,
        https_port = c.server_https_port,
        "Controller server is running."
    );

    let allocator_set = Arc::new(AllocatorSet::new());

    let metrics = Arc::new(Mutex::new(Metrics::default().register(&state.registry).unwrap()));

    // Start reconcilers
    tracing::info!("Start ClusterBGP reconciler");
    let cluster_bgp_state = state.clone();
    let cb_metrics = metrics.clone();
    tokio::spawn(async move {
        reconciler::cluster_bgp::run(cluster_bgp_state, c.requeue_interval, cb_metrics).await;
    });

    let client = Client::try_default()
        .await
        .expect("Failed to get kube client");
    let address_block_api = Api::<AddressBlock>::all(client.clone());
    let address_pool_api = Api::<AddressPool>::all(client.clone());
    let block_allocator = Arc::new(
        recover_or_create(&address_pool_api, &address_block_api)
            .await
            .unwrap(),
    );

    tracing::info!("Start AddressPool reconciler");
    let address_pool_state = state.clone();
    let ap_metrics = metrics.clone();
    let block_allocator_cloned = block_allocator.clone();
    tokio::spawn(async move {
        reconciler::address_pool::run(
            address_pool_state,
            c.requeue_interval,
            block_allocator_cloned,
            ap_metrics,
        )
        .await;
    });

    tracing::info!("Start AddressBlock reconciler");
    let address_block_state = state.clone();
    let ab_allocator_set = allocator_set.clone();
    let ab_block_allocator = block_allocator.clone();
    let ab_ctx = ControllerAddressBlockContext {
        allocator_set: ab_allocator_set,
        block_allocator: ab_block_allocator,
    };
    let ab_metrics = metrics.clone();
    tokio::spawn(async move {
        reconciler::address_block::run(address_block_state, c.requeue_interval, ab_ctx, ab_metrics)
            .await;
    });

    if c.mode.eq(&Mode::CNI) || c.mode.eq(&Mode::Dual) {
        tracing::info!("Start BlockRequest reconciler");
        let block_request_state = state.clone();
        let br_metrics = metrics.clone();
        tokio::spawn(async move {
            reconciler::block_request::run(
                block_request_state,
                c.requeue_interval,
                block_allocator.clone(),
                br_metrics,
            )
            .await;
        });
    }

    tracing::info!("Start Node watcher");
    let node_state = state.clone();
    let m_metrics = metrics.clone();
    tokio::spawn(async move {
        reconciler::node_watcher::run(node_state, c.requeue_interval, m_metrics).await;
    });

    if c.mode.eq(&Mode::LB) || c.mode.eq(&Mode::Dual) {
        tracing::info!("Start Service watcher");
        let service_state = state.clone();
        let svc_allocator_set = allocator_set.clone();
        let svc_metrics = metrics.clone();
        tokio::spawn(async move {
            reconciler::service_watcher::run(
                service_state,
                c.requeue_interval,
                svc_allocator_set,
                svc_metrics,
            )
            .await;
        });

        tracing::info!("Start Endpointslice watcher");
        let endpointslice_state = state.clone();
        let eps_metrics = metrics.clone();
        tokio::spawn(async move {
            reconciler::endpointslice_watcher::run(
                endpointslice_state,
                c.requeue_interval,
                eps_metrics,
            )
            .await;
        });
    }

    tracing::info!("Start BGPAdvertisement reconciler");
    let bgp_advertisement_state = state.clone();
    let ba_metrics = metrics.clone();
    tokio::spawn(async move {
        reconciler::bgp_advertisement::run(bgp_advertisement_state, c.requeue_interval, ba_metrics)
            .await;
    });

    server.run().await.unwrap()
}

pub struct Controller {
    server_port: u32,
    server_https_port: u32,
    server_cert: String,
    server_key: String,
    requeue_interval: u64,
    mode: Mode,
}

impl Controller {
    pub fn new(config: Config) -> Self {
        Self {
            server_port: config.http_port,
            server_https_port: config.https_port,
            server_cert: config.tls.cert,
            server_key: config.tls.key,
            requeue_interval: config.requeue_interval,
            mode: config.mode,
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
    tracing::info!(data=?buffer);
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

#[post("/validate-sart-terassyi-net-v1alpha2-addresspool")]
async fn address_pool_validating_webhook(
    req: HttpRequest,
    body: web::Json<AdmissionReview<AddressPool>>,
) -> impl Responder {
    webhook::address_pool::handle_validation(req, body).await
}

#[post("/mutate-v1-service")]
async fn service_mutating_webhook(
    req: HttpRequest,
    body: web::Json<AdmissionReview<Service>>,
) -> impl Responder {
    webhook::service::handle_mutation(req, body).await
}

#[post("/mutate-sart-terassyi-net-v1alpha2-addressblock")]
async fn address_block_mutating_webhook(
    req: HttpRequest,
    body: web::Json<AdmissionReview<AddressBlock>>,
) -> impl Responder {
    webhook::address_block::handle_mutation(req, body).await
}

async fn recover_or_create(
    address_pool_api: &Api<AddressPool>,
    address_block_api: &Api<AddressBlock>,
) -> Result<BlockAllocator, Error> {
    let ap_list = address_pool_api
        .list(&ListParams::default())
        .await
        .map_err(Error::Kube)?;
    let ab_list = address_block_api
        .list(&ListParams::default())
        .await
        .map_err(Error::Kube)?;

    let mut pool_map = ap_list
        .iter()
        .map(|ap| {
            let cidr = IpNet::from_str(&ap.spec.cidr)
                .map_err(|_| Error::InvalidCIDR)
                .unwrap(); // I want to return error in map()...
            let block_size = ap.spec.block_size.unwrap_or(cidr.prefix_len() as u32);
            (ap.name_any(), Pool::new(&cidr, block_size))
        })
        .collect::<HashMap<String, Pool>>();

    let mut pool_counter = HashMap::<String, u32>::new();
    for ab in ab_list.iter() {
        let ab_name = ab.name_any();
        let ab_name_splitted = ab_name.split('-');
        let c = ab_name_splitted.last();
        let c_num: u32 = match c {
            Some(c) => match c.parse() {
                Ok(c_n) => c_n,
                Err(_) => continue,
            },
            None => {
                continue;
            }
        };

        let pool_name = ab.spec.pool_ref.as_str();

        match pool_counter.get_mut(pool_name) {
            Some(c_max) => {
                if *c_max < c_num {
                    *c_max = c_num;
                }
            }
            None => {
                pool_counter.insert(pool_name.to_string(), c_num);
            }
        }

        if let Some(pool) = pool_map.get_mut(pool_name) {
            let block_index = match &ab.status {
                Some(status) => status.index,
                None => return Err(Error::AddressBlockIndexNotSet),
            };
            pool.allocate_with(block_index as u128, &ab.name_any())
                .map_err(Error::Ipam)?;
        }
    }

    // update pool counter
    for (pool_name, count) in pool_counter.iter() {
        if let Some(pool) = pool_map.get_mut(pool_name) {
            pool.counter = *count;
        }
    }

    Ok(BlockAllocator::new(pool_map))
}
