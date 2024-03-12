use actix_web::{http, web, HttpRequest, HttpResponse, Responder};
use kube::{
    api::ListParams,
    core::{
        admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
        response::StatusSummary,
        Status,
    },
    Api, Client, ResourceExt,
};

use crate::crd::address_pool::{AddressPool, MAX_BLOCK_SIZE};

#[tracing::instrument(skip_all)]
pub async fn handle_validation(
    req: HttpRequest,
    body: web::Json<AdmissionReview<AddressPool>>,
) -> impl Responder {
    tracing::info!(method=?req.method(), uri=?req.uri(), "Call validating webhook for AddressPool");

    if let Some(content_type) = req.head().headers.get("content-type") {
        if content_type != "application/json" {
            let msg = format!("invalid content-type: {:?}", content_type);

            return HttpResponse::BadRequest().json(msg);
        }
    }

    let admission_req: AdmissionRequest<AddressPool> = match body.into_inner().try_into() {
        Ok(req) => req,
        Err(e) => {
            tracing::error!(error=?e,"Invalid request");
            return HttpResponse::InternalServerError()
                .json(&AdmissionResponse::invalid(e.to_string()).into_review());
        }
    };

    let mut resp = AdmissionResponse::from(&admission_req);

    if let Some(ap) = admission_req.object {
        if ap.spec.block_size > MAX_BLOCK_SIZE {
            resp.allowed = false;
            resp.result = Status {
                status: Some(StatusSummary::Failure),
                code: http::StatusCode::FORBIDDEN.as_u16(),
                message: "Forbidden by validating webhook".to_string(),
                reason: format!("Block size must be lower than {}", MAX_BLOCK_SIZE),
                details: None,
            };
            return HttpResponse::Ok().json(resp.into_review());
        }
        if ap.spec.auto_assign.unwrap_or(false) {
            // In case, autoAssign field  is set as true, check if auto assignable AddressPoll already exists.
            let client = Client::try_default()
                .await
                .expect("Failed to create kube client");

            let address_pool_api = Api::<AddressPool>::all(client);
            match address_pool_api.list(&ListParams::default()).await {
                Ok(ap_list) => {
                    if ap_list.items.iter().any(|p| {
                        p.spec.auto_assign.unwrap_or(false)
                            && p.spec.r#type.eq(&ap.spec.r#type)
                            && p.name_any().ne(&ap.name_any())
                    }) {
                        tracing::warn!("Auto assignable AddressPool already exists.");
                        resp.allowed = false;
                        resp.result = Status {
                            status: Some(StatusSummary::Failure),
                            code: http::StatusCode::FORBIDDEN.as_u16(),
                            message: "Forbidden because Auto assignable address pool must be one."
                                .to_string(),
                            reason: "Auto assignable AddressPool already exists".to_string(),
                            details: None,
                        };
                        return HttpResponse::Ok().json(resp.into_review());
                    }
                }
                Err(e) => {
                    tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
                    return HttpResponse::InternalServerError()
                        .json(&AdmissionResponse::invalid(e.to_string()).into_review());
                }
            }
        }
    }

    resp.allowed = true;
    resp.result = Status {
        status: Some(StatusSummary::Success),
        code: http::StatusCode::OK.as_u16(),
        ..Default::default()
    };

    HttpResponse::Ok().json(resp.into_review())
}
