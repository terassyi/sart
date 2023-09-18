use actix_web::{HttpRequest, web, Responder, HttpResponse};
use kube::core::{Status, admission::{AdmissionReview, AdmissionResponse, AdmissionRequest}, response::{StatusSummary, StatusDetails}};
use tracing::instrument;
use tracing_subscriber::fmt::format::json;

use crate::reconcilers::bgppeer::BgpPeer;

pub(crate) const VALIDATION_WEBHOOK_PATH: &'static str = "validate-sart-terassyi-net-v1alpha2-bgppeer";


#[instrument(skip(req, body))]
pub(crate) async fn handle(req: HttpRequest, body: web::Json<AdmissionReview<BgpPeer>>) -> impl Responder {

	tracing::info!(method=?req.method(), uri=?req.uri(),"call validation webhook for BgpPeer");

	if let Some(content_type) = req.head().headers.get("content-type") {
		if content_type != "application/json" {
			let msg = format!("invalid content-type: {:?}", content_type);

			return HttpResponse::BadRequest().json(msg);
		}
	}

	let admission_req: AdmissionRequest<BgpPeer> = match body.into_inner().try_into() {
		Ok(req) => req,
		Err(e) => {
			tracing::error!("invalid request: {}", e);
			return HttpResponse::InternalServerError()
				.json(&AdmissionResponse::invalid(e.to_string()).into_review());
		}
	};

	let mut resp = AdmissionResponse::from(&admission_req);

	if admission_req.old_object.is_none() {
		tracing::info!(name=admission_req.name,"new object");
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

	tracing::info!(name=admission_req.name,"incomming request try to updates existing object");

	let old = admission_req.old_object.unwrap();

	if let Some(new) = admission_req.object {
		if new.spec.peer.local_addr != old.spec.peer.local_addr || 
			new.spec.peer.local_asn != old.spec.peer.local_asn ||
			new.spec.peer.local_name != old.spec.peer.local_name ||
			new.spec.peer.neighbor.addr != old.spec.peer.neighbor.addr ||
			new.spec.peer.neighbor.asn != old.spec.peer.neighbor.asn
		{
			let msg = "bgp session infomation(asn, address) must not be changed";
			tracing::error!(name=admission_req.name,msg);

			return HttpResponse::Forbidden().json(msg);
		}
	}

	resp.allowed = true;
	resp.result = Status {
		status: Some(StatusSummary::Success),
		message: "bgp session infomation is not modified".to_string(),
		code: 200,
		reason: "bgp session infomation is not modified".to_string(),
		details: None,
	};
	HttpResponse::Ok().json(resp.into_review())

}
