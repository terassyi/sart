use actix_web::{HttpRequest, web, Responder, HttpResponse};
use kube::core::{DynamicObject, admission::AdmissionReview};
use tracing::instrument;

pub(crate) const VALIDATION_WEBHOOK_PATH: &'static str = "validate-sart-terassyi-net-v1alpha2-bgppeer";


#[instrument(skip(req, body))]
pub(crate) async fn handle(req: HttpRequest, body: web::Json<AdmissionReview<DynamicObject>>) -> impl Responder {

	tracing::info!(method=?req.method(), uri=?req.uri(),"call validation webhook for BgpPeer");

	HttpResponse::Ok()
}
