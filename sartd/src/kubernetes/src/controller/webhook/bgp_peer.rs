use actix_web::{web, HttpRequest, HttpResponse, Responder};
use kube::{
    api::ListParams,
    core::{
        admission::{AdmissionRequest, AdmissionResponse, AdmissionReview},
        response::StatusSummary,
        Status,
    },
    Api, Client, ResourceExt,
};
use tracing::instrument;

use crate::{
    controller::error::Error,
    crd::bgp_peer::{BGPPeer, BGP_PEER_NODE_LABEL},
    util::escape_slash,
};

#[instrument(skip(req, body))]
pub async fn handle_validation(
    req: HttpRequest,
    body: web::Json<AdmissionReview<BGPPeer>>,
) -> impl Responder {
    tracing::info!(method=?req.method(), uri=?req.uri(),"call validating webhook for BGPPeer");

    if let Some(content_type) = req.head().headers.get("content-type") {
        if content_type != "application/json" {
            let msg = format!("invalid content-type: {:?}", content_type);

            return HttpResponse::BadRequest().json(msg);
        }
    }

    let admission_req: AdmissionRequest<BGPPeer> = match body.into_inner().try_into() {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("invalid request: {}", e);
            return HttpResponse::InternalServerError()
                .json(&AdmissionResponse::invalid(e.to_string()).into_review());
        }
    };

    let mut resp = AdmissionResponse::from(&admission_req);

    if admission_req.old_object.is_none() {
        if let Some(bp) = admission_req.object {
            if let Err(e) = validate_same_peer(&bp).await {
                tracing::error!(error=?e,name=admission_req.name, "Forbidden creating the peer");
                resp.allowed = false;
                resp.result = Status {
                    status: Some(StatusSummary::Failure),
                    message: e.to_string(),
                    code: 403,
                    reason: e.to_string(),
                    details: None,
                };

                return HttpResponse::Forbidden().json(resp);
            }
        }
        tracing::info!(name = admission_req.name, "new object");
        resp.allowed = true;
        resp.result = Status {
            status: Some(StatusSummary::Success),
            message: "new object".to_string(),
            code: 200,
            reason: "new object".to_string(),
            details: None,
        };
        return HttpResponse::Ok().json(resp.into_review());
    }

    tracing::info!(
        name = admission_req.name,
        "incoming request try to updates existing object"
    );

    let old = admission_req.old_object.unwrap();

    if let Some(new) = admission_req.object {
        if new.spec.asn != old.spec.asn
            || new.spec.addr != old.spec.addr
            || new.spec.node_bgp_ref != old.spec.node_bgp_ref
        {
            let msg =
                "bgp session information(asn, address, local bgp information) must not be changed";
            tracing::error!(name = admission_req.name, msg);

            return HttpResponse::Forbidden().json(msg);
        }
    }

    resp.allowed = true;
    resp.result = Status {
        status: Some(StatusSummary::Success),
        message: "bgp session information is not modified".to_string(),
        code: 200,
        reason: "bgp session information is not modified".to_string(),
        details: None,
    };
    HttpResponse::Ok().json(resp.into_review())
}

async fn validate_same_peer(bp: &BGPPeer) -> Result<(), Error> {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let bgp_peer_api = Api::<BGPPeer>::all(client);
    let bp_list = bgp_peer_api
        .list(&ListParams::default())
        .await
        .map_err(Error::Kube)?;

    for peer in bp_list.iter() {
        if bp.spec.asn == peer.spec.asn
            && bp.spec.addr.eq(&peer.spec.addr)
            && bp.spec.node_bgp_ref.eq(&peer.spec.node_bgp_ref)
        {
            return Err(Error::PeerAlreadyExists);
        }
    }
    Ok(())
}

#[tracing::instrument(skip_all)]
pub async fn handle_mutation(
    req: HttpRequest,
    body: web::Json<AdmissionReview<BGPPeer>>,
) -> impl Responder {
    tracing::info!(method=?req.method(), uri=?req.uri(),"call mutating webhook for BgpPeer");

    if let Some(content_type) = req.head().headers.get("content-type") {
        if content_type != "application/json" {
            let msg = format!("invalid content-type: {:?}", content_type);

            return HttpResponse::BadRequest().json(msg);
        }
    }

    let admission_req: AdmissionRequest<BGPPeer> = match body.into_inner().try_into() {
        Ok(req) => req,
        Err(e) => {
            tracing::error!("invalid request: {}", e);
            return HttpResponse::InternalServerError()
                .json(&AdmissionResponse::invalid(e.to_string()).into_review());
        }
    };

    let mut resp = AdmissionResponse::from(&admission_req);

    if let Some(bp) = admission_req.object {
        let name = bp.name_any();
        resp = match mutate_bgp_peer(&resp, &bp) {
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

fn mutate_bgp_peer(res: &AdmissionResponse, bp: &BGPPeer) -> Result<AdmissionResponse, Error> {
    match bp.labels().get(BGP_PEER_NODE_LABEL) {
        Some(n) => {
            if bp.spec.node_bgp_ref.ne(n) {
                return Err(Error::LabelMatching(format!(
                    "{} must be equal to spec.nodeBGPRef field",
                    BGP_PEER_NODE_LABEL
                )));
            }
            Ok(res.clone())
        }
        None => {
            let mut patches = Vec::new();
            if bp.metadata.labels.is_none() {
                let patch = json_patch::PatchOperation::Add(json_patch::AddOperation {
                    path: "/metadata/labels".into(),
                    value: serde_json::json!({}),
                });
                patches.push(patch);
            }

            let patch = json_patch::PatchOperation::Add(json_patch::AddOperation {
                path: format!("/metadata/labels/{}", escape_slash(BGP_PEER_NODE_LABEL)),
                value: serde_json::Value::String(bp.spec.node_bgp_ref.to_string()),
            });
            patches.push(patch);
            Ok(res
                .clone()
                .with_patch(json_patch::Patch(patches))
                .map_err(Error::SerializePatch)?)
        }
    }
}
