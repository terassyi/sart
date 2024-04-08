use actix_web::{web, HttpRequest, HttpResponse, Responder};
use k8s_openapi::api::core::v1::Service;
use kube::{
    core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
    ResourceExt,
};

use crate::{
    controller::reconciler::service_watcher::{
        get_allocated_lb_addrs, is_loadbalancer, RELEASE_ANNOTATION,
    },
    util::escape_slash,
};

#[tracing::instrument(skip_all)]
pub async fn handle_mutation(
    req: HttpRequest,
    body: web::Json<AdmissionReview<Service>>,
) -> impl Responder {

    if let Some(content_type) = req.head().headers.get("content-type") {
        if content_type != "application/json" {
            let msg = format!("invalid content-type: {:?}", content_type);

            return HttpResponse::BadRequest().json(msg);
        }
    }

    let admission_req: AdmissionRequest<Service> = match body.into_inner().try_into() {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("invalid request: {}", e);
            return HttpResponse::InternalServerError()
                .json(&AdmissionResponse::invalid(e.to_string()).into_review());
        }
    };

    let resp = AdmissionResponse::from(&admission_req);

    // object field must be set
    let new_svc = admission_req.object.unwrap();

    if is_loadbalancer(&new_svc) && new_svc.annotations().get(RELEASE_ANNOTATION).is_some() {
        let patches = vec![json_patch::PatchOperation::Remove(json_patch::RemoveOperation {
            path: format!("/metadata/annotations/{}", escape_slash(RELEASE_ANNOTATION)),
        })];
        let resp = match resp.with_patch(json_patch::Patch(patches)) {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!(error=?e,name=new_svc.name_any(), namespace=new_svc.namespace().unwrap(), "failed to handle request");
                return HttpResponse::InternalServerError()
                    .json("failed to handle a webhook request");
            }
        };
        return HttpResponse::Ok().json(resp.into_review());
    }

    if let Some(old_svc) = admission_req.old_object {
        if is_loadbalancer(&old_svc) && !is_loadbalancer(&new_svc) {
            tracing::info!(
                name = old_svc.name_any(),
                namespace = old_svc.namespace().unwrap(),
                "Request to change from LoadBalancer to other Service type",
            );
            match get_allocated_lb_addrs(&old_svc) {
                Some(allocated) => {
                    tracing::warn!(old=?old_svc);
                    let release = allocated
                        .iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<String>>()
                        .join(",");
                    tracing::info!(release=?release,"releasable");
                    let patches = vec![json_patch::PatchOperation::Add(json_patch::AddOperation {
                        path: format!("/metadata/annotations/{}", escape_slash(RELEASE_ANNOTATION)),
                        value: serde_json::Value::String(release),
                    })];
                    let resp = match resp.with_patch(json_patch::Patch(patches)) {
                        Ok(resp) => resp,
                        Err(e) => {
                            tracing::error!(error=?e,name=old_svc.name_any(), namespace=old_svc.namespace().unwrap(), "failed to handle request");
                            return HttpResponse::InternalServerError()
                                .json("failed to handle a webhook request");
                        }
                    };
                    return HttpResponse::Ok().json(resp.into_review());
                }
                None => {
                    // nothing to patch
                    return HttpResponse::Ok().json(resp.into_review());
                }
            }
        }
    }

    HttpResponse::Ok().json(resp.into_review())
}
