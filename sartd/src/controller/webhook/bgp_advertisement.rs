use actix_web::{web, HttpRequest, HttpResponse, Responder};
use kube::core::{
    admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
    response::StatusSummary,
    Status,
};

use crate::kubernetes::crd::bgp_advertisement::BGPAdvertisement;

#[tracing::instrument(skip_all)]
pub(crate) async fn handle_validation(
    req: HttpRequest,
    body: web::Json<AdmissionReview<BGPAdvertisement>>,
) -> impl Responder {
    tracing::info!(method=?req.method(), uri=?req.uri(), "call validating webhook for BGPAdvertisement");

    if let Some(content_type) = req.head().headers.get("content-type") {
        if content_type != "application/json" {
            let msg = format!("invalid content-type: {:?}", content_type);

            return HttpResponse::BadRequest().json(msg);
        }
    }
    let admission_req: AdmissionRequest<BGPAdvertisement> = match body.into_inner().try_into() {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("invalid request: {}", e);
            return HttpResponse::InternalServerError()
                .json(&AdmissionResponse::invalid(e.to_string()).into_review());
        }
    };

    let mut resp = AdmissionResponse::from(&admission_req);

    resp.allowed = true;
    resp.result = Status {
        status: Some(StatusSummary::Success),
        message: "nop".to_string(),
        code: 200,
        reason: "nop".to_string(),
        details: None,
    };
    HttpResponse::Ok().json(resp.into_review())
}
