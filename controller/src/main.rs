pub(crate) mod reconcilers;
pub(crate) mod context;
pub(crate) mod error;
pub(crate) mod metrics;
pub(crate) mod telemetry;
pub(crate) mod bgp;
pub(crate) mod proto;
pub(crate) mod speaker;



use actix_web::{HttpServer, App, web::Data, middleware, get, HttpRequest, Responder, HttpResponse};
use prometheus::{TextEncoder, Encoder};

use crate::context::State;


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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    telemetry::init().await;

    // Initiatilize Kubernetes controller state
    let state = State::default();

    // Preparer reconcilers for each resource
    let cluster_bgp_reconciler= reconcilers::clusterbgp::run(state.clone());
    let bgp_peer_reconciler = reconcilers::bgppeer::run(state.clone());
    let address_pool_reconciler = reconcilers::addresspool::run(state.clone());
    let bgp_advertisement_reconciler = reconcilers::bgpadvertisement::run(state.clone());


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
