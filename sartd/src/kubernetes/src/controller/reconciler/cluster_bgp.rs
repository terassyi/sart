use std::{collections::BTreeMap, net::Ipv4Addr, sync::Arc};

use futures::StreamExt;
use k8s_openapi::api::core::v1::Node;
use kube::{
    api::{ListParams, PostParams},
    core::{ApiResource, ObjectMeta},
    runtime::{
        controller::{Action, Controller},
        finalizer::{finalizer, Event},
        watcher::Config,
    },
    Api, Client, Resource, ResourceExt,
};

use crate::{
    context::{error_policy, Context, State},
    controller::error::Error,
    crd::{
        bgp_peer::PeerConfig,
        bgp_peer_template::BGPPeerTemplate,
        cluster_bgp::{
            AsnSelectionType, AsnSelector, ClusterBGP, RouterIdSelectionType, RouterIdSelector,
            ASN_LABEL, CLUSTER_BGP_FINALIZER, ROUTER_ID_LABEL,
        },
        node_bgp::{NodeBGP, NodeBGPSpec},
    },
    util::create_owner_reference,
};

#[tracing::instrument(skip_all, fields(trace_id))]
pub async fn reconciler(cb: Arc<ClusterBGP>, ctx: Arc<Context>) -> Result<Action, Error> {
    let cluster_bgps = Api::<ClusterBGP>::all(ctx.client.clone());

    finalizer(&cluster_bgps, CLUSTER_BGP_FINALIZER, cb, |event| async {
        match event {
            Event::Apply(cb) => reconcile(&cb, ctx.clone()).await,
            Event::Cleanup(cb) => cleanup(&cb, ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::Finalizer(Box::new(e)))
}

// reconcile() is called when a resource is applied or updated
#[tracing::instrument(skip_all)]
async fn reconcile(cb: &ClusterBGP, ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(name = cb.name_any(), "reconcile ClusterBGP");

    let nodes = Api::<Node>::all(ctx.client.clone());
    let list_params = match &cb.spec.node_selector {
        Some(selector) => ListParams::default().labels(&get_label_selector(selector)),
        None => ListParams::default(),
    };

    let matched_nodes = nodes.list(&list_params).await.map_err(Error::Kube)?;

    let node_bgps = Api::<NodeBGP>::all(ctx.client.clone());

    for node in matched_nodes.items.iter() {
        let asn = get_asn(node, &cb.spec.asn_selector)?;
        let router_id = get_router_id(node, &cb.spec.router_id_selector)?;

        match node_bgps
            .get_opt(&node.name_any())
            .await
            .map_err(Error::Kube)?
        {
            Some(nb) => {
                let mut new_nb = nb.clone();
                let mut need_upate = false;
                match &nb.meta().owner_references {
                    Some(owner_references) => {
                        let res = ApiResource::erase::<ClusterBGP>(&());
                        if owner_references.iter().any(|o| {
                            o.kind.eq(&res.kind)
                                && o.api_version.eq(&res.api_version)
                                && o.name.eq(&cb.name_any())
                        }) {
                            // In this line, owner_references must not be None
                            let mut new_owner_references = owner_references.clone();
                            new_owner_references.push(create_owner_reference(cb));
                            new_nb.metadata.owner_references = Some(new_owner_references);
                            need_upate = true;
                        }
                    }
                    None => {
                        new_nb.metadata.owner_references = Some(vec![create_owner_reference(cb)]);
                        need_upate = true;
                    }
                }
                if need_upate {
                    tracing::info!(node_bgp=nb.name_any(),asn=nb.spec.asn,router_id=?nb.spec.router_id,"Update existing NodeBGP's ownerReference");
                    node_bgps
                        .replace(&nb.name_any(), &PostParams::default(), &new_nb)
                        .await
                        .map_err(Error::Kube)?;
                }
            }
            None => {
                let metadata = ObjectMeta {
                    name: Some(node.name_any()),
                    labels: Some(node.labels().clone()),
                    owner_references: Some(vec![create_owner_reference(cb)]),
                    ..Default::default()
                };

                let node_name = node.name_any();
                let peer_templ_api = Api::<BGPPeerTemplate>::all(ctx.client.clone());

                let peer_configs: Vec<PeerConfig> = match &cb.spec.peers {
                    Some(peer_config) => peer_config
                        .iter()
                        .filter(|&p| match &p.node_bgp_selector {
                            Some(selector) => match_selector(node.labels(), selector),
                            None => true,
                        })
                        .cloned()
                        .collect(),
                    None => Vec::new(),
                };

                let mut peers = Vec::new();
                for c in peer_configs.iter() {
                    let p = c
                        .to_peer(&node_name, &peer_templ_api)
                        .await
                        .map_err(Error::CRD)?;
                    peers.push(p);
                }

                let node_bgp = NodeBGP {
                    metadata,
                    spec: NodeBGPSpec {
                        asn,
                        router_id: router_id.to_string(),
                        speaker: cb.spec.speaker.clone(),
                        peers: if peers.is_empty() { None } else { Some(peers) },
                    },
                    status: None,
                };
                tracing::info!(node_bgp=node.name_any(),asn=asn,router_id=?router_id,"Create new NodeBGP resource");
                node_bgps
                    .create(&PostParams::default(), &node_bgp)
                    .await
                    .map_err(Error::Kube)?;
            }
        }
    }

    Ok(Action::await_change())
}

// cleanup() is called when a resource is deleted
#[tracing::instrument(skip_all)]
async fn cleanup(cb: &ClusterBGP, _ctx: Arc<Context>) -> Result<Action, Error> {
    tracing::info!(name = cb.name_any(), "clean up ClusterBGP");

    Ok(Action::await_change())
}

pub async fn run(state: State, interval: u64) {
    let client = Client::try_default()
        .await
        .expect("Failed to create kube client");

    let cluster_bgps = Api::<ClusterBGP>::all(client.clone());
    if let Err(e) = cluster_bgps.list(&ListParams::default().limit(1)).await {
        tracing::error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        tracing::info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    tracing::info!("Start ClusterBgp reconciler");

    Controller::new(cluster_bgps, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(
            reconciler,
            error_policy::<ClusterBGP, Error, Context>,
            state.to_context(client, interval),
        )
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

fn get_label_selector(selector: &BTreeMap<String, String>) -> String {
    selector
        .iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<String>>()
        .join(",")
}

fn get_asn(node: &Node, selector: &AsnSelector) -> Result<u32, Error> {
    match selector.from {
        AsnSelectionType::Label => node
            .labels()
            .get(ASN_LABEL)
            .ok_or(Error::AsnNotFound)?
            .parse()
            .map_err(|_| Error::InvalidAsnValue),
        AsnSelectionType::Asn => selector.asn.ok_or(Error::InvalidAsnValue),
    }
}

fn get_router_id(node: &Node, selector: &RouterIdSelector) -> Result<Ipv4Addr, Error> {
    match selector.from {
        RouterIdSelectionType::RouterId => selector
            .router_id
            .clone()
            .ok_or(Error::AddressNotFound)?
            .parse::<Ipv4Addr>()
            .map_err(|_| Error::InvalidAddress),
        RouterIdSelectionType::InternalAddress => node
            .status
            .clone()
            .ok_or(Error::FailedToGetData("node.status".to_string()))?
            .addresses
            .ok_or(Error::FailedToGetData("node.status.addresses".to_string()))?
            .iter()
            .find(|&na| na.type_.eq("InternalIP"))
            .ok_or(Error::AddressNotFound)?
            .address
            .parse()
            .map_err(|_| Error::InvalidAddress),
        RouterIdSelectionType::Label => node
            .labels()
            .get(ROUTER_ID_LABEL)
            .ok_or(Error::AddressNotFound)?
            .parse()
            .map_err(|_| Error::InvalidAddress),
    }
}

fn match_selector(labels: &BTreeMap<String, String>, selector: &BTreeMap<String, String>) -> bool {
    for (k, v) in selector.iter() {
        match labels.get_key_value(k) {
            Some((_mk, mv)) => {
                if mv.ne(v) {
                    return false;
                }
            }
            None => return false,
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use http::Response;
    use hyper::Body;
    use k8s_openapi::api::core::v1::Node;
    use kube::core::ObjectMeta;
    use kube::ResourceExt;
    use rstest::rstest;
    use std::collections::BTreeMap;

    use crate::context::Context;
    use crate::controller::error::Error;
    use crate::crd::bgp_peer::PeerConfig;
    use crate::crd::bgp_peer_template::BGPPeerTemplate;
    use crate::crd::bgp_peer_template::BGPPeerTemplateSpec;
    use crate::crd::cluster_bgp::AsnSelectionType;
    use crate::crd::cluster_bgp::AsnSelector;
    use crate::crd::cluster_bgp::ClusterBGP;
    use crate::crd::cluster_bgp::ClusterBGPSpec;
    use crate::crd::cluster_bgp::RouterIdSelectionType;
    use crate::crd::cluster_bgp::RouterIdSelector;
    use crate::crd::cluster_bgp::SpeakerConfig;
    use crate::crd::node_bgp::NodeBGP;
    use crate::crd::node_bgp::NodeBGPSpec;
    use crate::fixture::reconciler::api_server_response_not_found;
    use crate::fixture::reconciler::api_server_response_resource;
    use crate::fixture::reconciler::assert_resource_request;
    use crate::fixture::reconciler::test_cluster_bgp;
    use crate::fixture::reconciler::test_node_list;
    use crate::fixture::reconciler::timeout_after_1s;
    use crate::fixture::reconciler::ApiServerVerifier;
    use crate::fixture::reconciler::TestBgpSelector;
    use crate::util::create_owner_reference;

    use super::get_label_selector;
    use super::match_selector;
    use super::reconcile;

    #[rstest(
		selector,
		expected,
		case(BTreeMap::from([]), ""),
		case(BTreeMap::from([("bgp".to_string(), "a".to_string())]), "bgp=a"),
		case(BTreeMap::from([("bgp".to_string(), "a".to_string()), ("test".to_string(), "b".to_string())]), "bgp=a,test=b"),
	)]
    fn test_get_label_selector(selector: BTreeMap<String, String>, expected: &str) {
        let res = get_label_selector(&selector);
        assert_eq!(res, expected);
    }

    #[rstest(
        labels,
        selector,
        expected,
        case(BTreeMap::from([]), BTreeMap::from([]), true),
        case(BTreeMap::from([("bgp".to_string(), "a".to_string())]), BTreeMap::from([]), true),
        case(BTreeMap::from([]), BTreeMap::from([("bgp".to_string(), "a".to_string())]), false),
        case(BTreeMap::from([("bgp".to_string(), "b".to_string())]), BTreeMap::from([("bgp".to_string(), "a".to_string())]), false),
        case(BTreeMap::from([("bgp".to_string(), "b".to_string()), ("peer".to_string(), "c".to_string())]), BTreeMap::from([("bgp".to_string(), "b".to_string())]), true),
        case(BTreeMap::from([("bgp".to_string(), "b".to_string()), ("peer".to_string(), "c".to_string())]), BTreeMap::from([("bgp".to_string(), "b".to_string()), ("peer".to_string(), "d".to_string())]), false),
    )]
    fn test_match_selector(
        labels: BTreeMap<String, String>,
        selector: BTreeMap<String, String>,
        expected: bool,
    ) {
        let res = match_selector(&labels, &selector);
        assert_eq!(res, expected);
    }

    enum Scenario {
        Create(ClusterBGP),
        Update(ClusterBGP),
    }

    impl ApiServerVerifier {
        fn cluster_bgp_run(self, scenario: Scenario) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async move {
                match scenario {
                    Scenario::Create(cb) => {
                        self.list_node(&cb)
                            .await
                            .unwrap()
                            .create_node_bgps(&cb)
                            .await
                    }
                    Scenario::Update(cb) => {
                        self.list_node(&cb)
                            .await
                            .unwrap()
                            .update_node_bgps(&cb)
                            .await
                    }
                }
                .expect("reconcile completed without error");
            })
        }

        async fn list_node(mut self, cb: &ClusterBGP) -> Result<Self, Error> {
            let (request, send) = self.0.next_request().await.expect("service not called");
            assert_resource_request(
                &request,
                &Node::default(),
                None,
                true,
                get_node_selector_from_cb(cb),
                http::Method::GET,
            );
            send.send_response(
                Response::builder()
                    .body(Body::from(
                        serde_json::to_vec(&test_node_list(TestBgpSelector::All)).unwrap(),
                    ))
                    .unwrap(),
            );
            Ok(self)
        }

        async fn create_node_bgps(mut self, cb: &ClusterBGP) -> Result<Self, Error> {
            let test_bgp_selector = Self::get_test_bgp_selector(cb);
            for node in test_node_list(test_bgp_selector) {
                let (request, send) = self.0.next_request().await.expect("service not called");
                let nb = NodeBGP {
                    metadata: ObjectMeta {
                        name: Some(node.name_any()),
                        ..Default::default()
                    },
                    spec: NodeBGPSpec::default(),
                    status: None,
                };
                assert_resource_request(&request, &nb, None, false, None, http::Method::GET);
                send.send_response(
                    Response::builder()
                        // .body(Body::from(api_server_response_resource(&nb)))
                        .status(http::StatusCode::NOT_FOUND)
                        .body(Body::from(api_server_response_not_found(&nb)))
                        .unwrap(),
                );

                if let Some(need_call_req) = cb
                    .spec
                    .peers
                    .clone()
                    .map(|peers| peers.into_iter().filter_map(|p| p.peer_template_ref))
                {
                    for p in need_call_req {
                        let (request, send) =
                            self.0.next_request().await.expect("service not called");
                        let peer_tmpl = BGPPeerTemplate {
                            metadata: ObjectMeta {
                                name: Some(p),
                                ..Default::default()
                            },
                            spec: BGPPeerTemplateSpec::default(),
                            status: None,
                        };
                        assert_resource_request(
                            &request,
                            &peer_tmpl,
                            None,
                            false,
                            None,
                            http::Method::GET,
                        );
                        send.send_response(
                            Response::builder()
                                .body(Body::from(api_server_response_resource(&peer_tmpl)))
                                .unwrap(),
                        );
                    }
                }
                let (request, send) = self.0.next_request().await.expect("service not called");
                assert_resource_request(&request, &nb, None, false, None, http::Method::POST);
                send.send_response(
                    Response::builder()
                        .body(Body::from(api_server_response_resource(&nb)))
                        .unwrap(),
                );
            }
            Ok(self)
        }

        async fn update_node_bgps(mut self, cb: &ClusterBGP) -> Result<Self, Error> {
            let test_bgp_selector = Self::get_test_bgp_selector(cb);
            for node in test_node_list(test_bgp_selector) {
                let (request, send) = self.0.next_request().await.expect("service not called");
                let mut nb = NodeBGP {
                    metadata: ObjectMeta {
                        name: Some(node.name_any()),
                        ..Default::default()
                    },
                    spec: NodeBGPSpec::default(),
                    status: None,
                };
                assert_resource_request(&request, &nb, None, false, None, http::Method::GET);
                send.send_response(
                    Response::builder()
                        .body(Body::from(api_server_response_resource(&nb)))
                        .unwrap(),
                );

                // NodeBGP's owner reference shouldn't be created here
                // Create it
                nb.owner_references_mut().push(create_owner_reference(cb));
                let (request, send) = self.0.next_request().await.expect("service not called");
                assert_resource_request(&request, &nb, None, false, None, http::Method::PUT);
                send.send_response(
                    Response::builder()
                        .body(Body::from(api_server_response_resource(&nb)))
                        .unwrap(),
                );
            }
            Ok(self)
        }

        fn get_test_bgp_selector(cb: &ClusterBGP) -> TestBgpSelector {
            match cb.labels().get("bgp") {
                Some(v) => match v.as_str() {
                    "a" => TestBgpSelector::A,
                    "b" => TestBgpSelector::B,
                    _ => panic!("unhandlable bgp selector value"),
                },
                None => TestBgpSelector::All,
            }
        }
    }

    fn get_node_selector_from_cb(cb: &ClusterBGP) -> Option<String> {
        // escape '=' to %3D (url encode)
        cb.spec
            .node_selector
            .clone()
            .and_then(|s| s.get("bgp").map(|v| format!("&labelSelector=bgp%3D{v}")))
    }

    #[tokio::test]
    async fn create_cluster_bgp_match_all() {
        let (testctx, fakeserver, _) = Context::test();
        let cb = test_cluster_bgp();

        let mocksvr = fakeserver.cluster_bgp_run(Scenario::Create(cb.clone()));
        reconcile(&cb, testctx).await.expect("reconciler");
        timeout_after_1s(mocksvr).await;
    }

    #[tokio::test]
    async fn create_cluster_bgp_match_selector_a() {
        let (testctx, fakeserver, _) = Context::test();
        let cb = ClusterBGP {
            metadata: ObjectMeta {
                name: Some("test-cb".to_string()),
                ..Default::default()
            },
            spec: ClusterBGPSpec {
                node_selector: Some(BTreeMap::from([("bgp".to_string(), "a".to_string())])),
                asn_selector: AsnSelector {
                    from: AsnSelectionType::Asn,
                    asn: Some(65000),
                },
                router_id_selector: RouterIdSelector {
                    from: RouterIdSelectionType::InternalAddress,
                    router_id: None,
                },
                speaker: SpeakerConfig {
                    path: "localhost:5000".to_string(),
                    timeout: None,
                },
                peers: Some(vec![PeerConfig {
                    peer_template_ref: Some("test-bgp-peer-templ".to_string()),
                    peer_config: None,
                    node_bgp_selector: None,
                }]),
            },
            status: None,
        };

        let mocksvr = fakeserver.cluster_bgp_run(Scenario::Create(cb.clone()));
        reconcile(&cb, testctx).await.expect("reconciler");
        timeout_after_1s(mocksvr).await;
    }

    #[tokio::test]
    async fn update_cluster_bgp_no_owner_reference() {
        let (testctx, fakeserver, _) = Context::test();
        let cb = ClusterBGP {
            metadata: ObjectMeta {
                name: Some("test-cb".to_string()),
                ..Default::default()
            },
            spec: ClusterBGPSpec {
                node_selector: None,
                asn_selector: AsnSelector {
                    from: AsnSelectionType::Asn,
                    asn: Some(65000),
                },
                router_id_selector: RouterIdSelector {
                    from: RouterIdSelectionType::InternalAddress,
                    router_id: None,
                },
                speaker: SpeakerConfig {
                    path: "localhost:5000".to_string(),
                    timeout: None,
                },
                peers: Some(vec![PeerConfig {
                    peer_template_ref: Some("test-bgp-peer-templ".to_string()),
                    peer_config: None,
                    node_bgp_selector: None,
                }]),
            },
            status: None,
        };

        let mocksvr = fakeserver.cluster_bgp_run(Scenario::Update(cb.clone()));
        reconcile(&cb, testctx).await.expect("reconciler");
        timeout_after_1s(mocksvr).await;
    }
}
