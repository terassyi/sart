use actix_web::{web, HttpRequest, HttpResponse, Responder};
use kube::{
    core::admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
    ResourceExt,
};

use crate::{
    controller::error::Error,
    crd::{
        address_block::{AddressBlock, ADDRESS_BLOCK_NODE_LABEL},
        address_pool::AddressType,
    },
    util::escape_slash,
};

#[tracing::instrument(skip_all)]
pub async fn handle_mutation(
    req: HttpRequest,
    body: web::Json<AdmissionReview<AddressBlock>>,
) -> impl Responder {

    if let Some(content_type) = req.head().headers.get("content-type") {
        if content_type != "application/json" {
            let msg = format!("invalid content-type: {:?}", content_type);

            return HttpResponse::BadRequest().json(msg);
        }
    }

    let admission_req: AdmissionRequest<AddressBlock> = match body.into_inner().try_into() {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("invalid request: {}", e);
            return HttpResponse::InternalServerError()
                .json(&AdmissionResponse::invalid(e.to_string()).into_review());
        }
    };
    let mut resp = AdmissionResponse::from(&admission_req);

    if let Some(ab) = admission_req.object {
        if ab.spec.r#type.ne(&AddressType::Pod) {
            return HttpResponse::Ok().json(resp.into_review());
        }
        let name = ab.name_any();
        resp = match mutate_address_block(&resp, &ab) {
            Ok(res) => {
                tracing::info!(
                    op=?admission_req.operation,
                    name=name,
                    "Accepted by mutating webhook",
                );
                res
            }
            Err(e) => {
                tracing::warn!(
                    op=?admission_req.operation,
                    name=name,
                    "Denied by mutating webhook",
                );
                resp.deny(e.to_string())
            }
        };
    }

    HttpResponse::Ok().json(resp.into_review())
}

fn mutate_address_block(
    res: &AdmissionResponse,
    ab: &AddressBlock,
) -> Result<AdmissionResponse, Error> {
    match ab.spec.node_ref.as_ref() {
        Some(node) => {
            let patch = if ab.labels().get(ADDRESS_BLOCK_NODE_LABEL).is_some() {
                json_patch::PatchOperation::Replace(json_patch::ReplaceOperation {
                    path: format!(
                        "/metadata/labels/{}",
                        escape_slash(ADDRESS_BLOCK_NODE_LABEL)
                    ),
                    value: serde_json::Value::String(node.to_string()),
                })
            } else {
                json_patch::PatchOperation::Add(json_patch::AddOperation {
                    path: format!(
                        "/metadata/labels/{}",
                        escape_slash(ADDRESS_BLOCK_NODE_LABEL)
                    ),
                    value: serde_json::Value::String(node.to_string()),
                })
            };
            Ok(res
                .clone()
                .with_patch(json_patch::Patch(vec![patch]))
                .map_err(Error::SerializePatch)?)
        }
        None => Err(Error::InvalidParameter(
            "spec.nodeRef is required for AddressBlock".to_string(),
        )),
    }
}
