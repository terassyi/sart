// #[cfg(test)]
pub mod reconciler {
    use std::{collections::BTreeMap, sync::Arc};

    use http::{Request, Response};
    use hyper::Body;
    use k8s_openapi::{
        api::{
            core::v1::{
                LoadBalancerIngress, LoadBalancerStatus, Node, NodeAddress, NodeStatus, Service,
                ServicePort, ServiceSpec, ServiceStatus,
            },
            discovery::v1::{Endpoint, EndpointConditions, EndpointSlice},
        },
        apimachinery::pkg::util::intstr::IntOrString,
    };
    use kube::{
        core::{ListMeta, ObjectList, ObjectMeta},
        discovery::ApiResource,
        Client, Resource, ResourceExt,
    };
    use prometheus::Registry;
    use sartd_trace::metrics::Metrics;
    use serde::Serialize;

    use crate::{
        context::{Context, ContextWith},
        controller::reconciler::{
            endpointslice_watcher::ENDPOINTSLICE_FINALIZER, service_watcher::SERVICE_FINALIZER,
        },
        crd::{
            address_block::{AddressBlock, AddressBlockSpec, ADDRESS_BLOCK_FINALIZER},
            address_pool::{
                AddressPool, AddressPoolSpec, AddressType, AllocationType, ADDRESS_POOL_FINALIZER,
            },
            bgp_advertisement::{
                BGPAdvertisement, BGPAdvertisementSpec, Protocol, BGP_ADVERTISEMENT_FINALIZER,
            },
            bgp_peer::{BGPPeer, BGPPeerSlim, BGPPeerSpec, PeerConfig},
            bgp_peer_template::{BGPPeerTemplate, BGPPeerTemplateSpec},
            block_request::{BlockRequest, BlockRequestSpec, BLOCK_REQUEST_FINALIZER},
            cluster_bgp::{
                AsnSelectionType, AsnSelector, ClusterBGP, ClusterBGPSpec, RouterIdSelectionType,
                RouterIdSelector, SpeakerConfig, CLUSTER_BGP_FINALIZER,
            },
            node_bgp::{NodeBGP, NodeBGPSpec},
        },
    };

    pub type ApiServerHandle = tower_test::mock::Handle<Request<Body>, Response<Body>>;
    pub struct ApiServerVerifier(pub ApiServerHandle);

    pub async fn timeout_after_1s(handle: tokio::task::JoinHandle<()>) {
        tokio::time::timeout(std::time::Duration::from_secs(1), handle)
            .await
            .expect("timeout on mock apiserver")
            .expect("scenario succeeded")
    }

    impl Context {
        pub fn test() -> (Arc<Self>, ApiServerVerifier, Registry) {
            let (mock_service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
            let mock_client = Client::new(mock_service, "default");
            let registry = Registry::default();
            let ctx = Self {
                client: mock_client,
                metrics: Metrics::default().register(&registry).unwrap(),
                diagnostics: Arc::default(),
                interval: 30,
            };
            (Arc::new(ctx), ApiServerVerifier(handle), registry)
        }
    }

    impl<T: Clone> ContextWith<T> {
        pub fn test(component: T) -> (Arc<Self>, ApiServerVerifier, Registry) {
            let (mock_service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
            let mock_client = Client::new(mock_service, "default");
            let registry = Registry::default();
            let ctx = Context {
                client: mock_client,
                metrics: Metrics::default().register(&registry).unwrap(),
                diagnostics: Arc::default(),
                interval: 30,
            };
            let ctx_with = Self {
                inner: ctx,
                component,
            };
            (Arc::new(ctx_with), ApiServerVerifier(handle), registry)
        }
    }

    fn base_uri<T: Resource<DynamicType = ()>>() -> String {
        let api_resource = ApiResource::erase::<T>(&());
        if api_resource.group.eq("sart.terassyi.net") {
            format!("/apis/{}", api_resource.api_version)
        } else {
            format!("/api/{}", api_resource.api_version)
        }
    }

    pub fn get_uri<T: Resource<DynamicType = ()>>(res: &T) -> String {
        let api_resource = ApiResource::erase::<T>(&());
        match res.namespace() {
            Some(ns) => {
                format!(
                    "{}/namespaces/{}/{}/{}",
                    base_uri::<T>(),
                    ns,
                    api_resource.plural,
                    res.name_any()
                )
            }
            None => {
                format!(
                    "{}/{}/{}",
                    base_uri::<T>(),
                    // api_resource.kind,
                    api_resource.plural,
                    res.name_any()
                )
            }
        }
    }

    pub fn post_uri<T: Resource<DynamicType = ()>>(res: &T) -> String {
        let api_resource = ApiResource::erase::<T>(&());
        match res.namespace() {
            Some(ns) => {
                format!(
                    "{}/namespaces/{}/{}?",
                    base_uri::<T>(),
                    ns,
                    api_resource.plural,
                )
            }
            None => {
                format!("{}/{}?", base_uri::<T>(), api_resource.plural,)
            }
        }
    }

    pub fn put_uri<T: Resource<DynamicType = ()>>(res: &T, subresource: Option<&str>) -> String {
        let api_resource = ApiResource::erase::<T>(&());
        let sub = match subresource {
            Some(s) => format!("/{s}"),
            None => String::new(),
        };
        match res.namespace() {
            Some(ns) => {
                format!(
                    "{}/namespaces/{}/{}/{}{}?",
                    base_uri::<T>(),
                    ns,
                    api_resource.plural,
                    res.name_any(),
                    sub,
                )
            }
            None => {
                format!(
                    "{}/{}/{}{}?",
                    base_uri::<T>(),
                    api_resource.plural,
                    res.name_any(),
                    sub,
                )
            }
        }
    }

    fn list_uri<T: Resource<DynamicType = ()>>(res: &T) -> String {
        let api_resource = ApiResource::erase::<T>(&());
        match res.namespace() {
            Some(ns) => {
                format!(
                    "{}/namespaces/{}/{}?",
                    base_uri::<T>(),
                    ns,
                    api_resource.plural,
                )
            }
            None => {
                format!("{}/{}?", base_uri::<T>(), api_resource.plural,)
            }
        }
    }

    fn patch_uri<T: Resource<DynamicType = ()>>(res: &T) -> String {
        format!("{}?", get_uri(res))
    }

    pub fn assert_resource_request<T: Resource<DynamicType = ()>>(
        request: &Request<Body>,
        res: &T,
        subresource: Option<&str>,
        list: bool,
        label_selector: Option<String>,
        method: http::Method,
    ) {
        assert_eq!(request.method(), method);
        let uri = match method {
            http::Method::GET => {
                if list {
                    list_uri(res)
                } else {
                    get_uri(res)
                }
            }
            http::Method::POST => post_uri(res),
            http::Method::PUT => put_uri(res, subresource),
            http::Method::PATCH => patch_uri(res),
            _ => panic!("unimplemented method"),
        };
        let uri = if let Some(selector) = label_selector {
            format!("{uri}{}", selector)
        } else {
            uri
        };
        assert_eq!(request.uri().to_string(), uri);
    }

    pub fn api_server_response_not_found<T: Resource<DynamicType = ()>>(res: &T) -> String {
        let api_resource = ApiResource::erase::<T>(&());
        let (group_kind, details) = if api_resource.group.eq("sart.terassyi.net") {
            (
                format!("{}.{}", api_resource.plural, api_resource.group),
                format!(r#""group": "{}","#, api_resource.group),
            )
        } else {
            (api_resource.plural.clone(), String::new())
        };
        format!(
            r#"{{
  "kind": "Status",
  "apiVersion": "v1",
  "metadata": {{}},
  "status": "Failure",
  "message": "{} \"{}\" not found",
  "reason": "NotFound",
  "details": {{
    "name": "{}",
    {}
    "kind": "{}"
  }},
  "code": 404
}}"#,
            group_kind,
            res.name_any(),
            res.name_any(),
            details,
            api_resource.plural,
        )
    }

    pub fn api_server_response_resource<T: Resource<DynamicType = ()> + Serialize>(
        res: &T,
    ) -> Vec<u8> {
        serde_json::to_vec(res).unwrap()
    }

    pub enum TestBgpSelector {
        A,
        B,
        All,
    }

    pub fn test_address_pool_lb() -> AddressPool {
        AddressPool {
            metadata: ObjectMeta {
                name: Some("test-pool".to_string()),
                finalizers: Some(vec![ADDRESS_POOL_FINALIZER.to_string()]),
                ..Default::default()
            },
            spec: AddressPoolSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: AddressType::Service,
                alloc_type: Some(AllocationType::Bit),
                block_size: 16,
                auto_assign: Some(true),
            },
            status: None,
        }
    }

    pub fn test_address_block_lb() -> AddressBlock {
        AddressBlock {
            metadata: ObjectMeta {
                finalizers: Some(vec![ADDRESS_BLOCK_FINALIZER.to_string()]),
                name: Some("test-pool".to_string()),
                ..Default::default()
            },
            spec: AddressBlockSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: AddressType::Service,
                pool_ref: "test-pool".to_string(),
                node_ref: None,
                auto_assign: true,
            },
            status: None,
        }
    }

    pub fn test_address_block_lb_non_default() -> AddressBlock {
        AddressBlock {
            metadata: ObjectMeta {
                finalizers: Some(vec![ADDRESS_BLOCK_FINALIZER.to_string()]),
                name: Some("test-pool-non-default".to_string()),
                ..Default::default()
            },
            spec: AddressBlockSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: AddressType::Service,
                pool_ref: "test-pool".to_string(),
                node_ref: None,
                auto_assign: false,
            },
            status: None,
        }
    }

    pub fn test_address_pool_pod() -> AddressPool {
        AddressPool {
            metadata: ObjectMeta {
                name: Some("test-pool".to_string()),
                finalizers: Some(vec![ADDRESS_POOL_FINALIZER.to_string()]),
                ..Default::default()
            },
            spec: AddressPoolSpec {
                cidr: "10.0.0.0/16".to_string(),
                r#type: AddressType::Pod,
                alloc_type: Some(AllocationType::Bit),
                block_size: 27,
                auto_assign: Some(true),
            },
            status: None,
        }
    }

    pub fn test_address_pool_pod_another() -> AddressPool {
        AddressPool {
            metadata: ObjectMeta {
                name: Some("test-pool".to_string()),
                finalizers: Some(vec![ADDRESS_POOL_FINALIZER.to_string()]),
                ..Default::default()
            },
            spec: AddressPoolSpec {
                cidr: "10.0.0.0/30".to_string(),
                r#type: AddressType::Pod,
                alloc_type: Some(AllocationType::Bit),
                block_size: 31,
                auto_assign: Some(true),
            },
            status: None,
        }
    }

    pub fn test_address_block_pod() -> AddressBlock {
        AddressBlock {
            metadata: ObjectMeta {
                finalizers: Some(vec![ADDRESS_BLOCK_FINALIZER.to_string()]),
                name: Some("test-pool-sart-integration-control-plane-10.0.0.0".to_string()),
                ..Default::default()
            },
            spec: AddressBlockSpec {
                cidr: "10.0.0.0/27".to_string(),
                r#type: AddressType::Pod,
                pool_ref: "test-pool".to_string(),
                node_ref: Some("sart-integration-control-plane".to_string()),
                auto_assign: true,
            },
            status: None,
        }
    }

    pub fn test_address_block_pod2() -> AddressBlock {
        AddressBlock {
            metadata: ObjectMeta {
                finalizers: Some(vec![ADDRESS_BLOCK_FINALIZER.to_string()]),
                name: Some("test-pool-sart-integration-control-plane-10.0.0.32".to_string()),
                ..Default::default()
            },
            spec: AddressBlockSpec {
                cidr: "10.0.0.32/27".to_string(),
                r#type: AddressType::Pod,
                pool_ref: "test-pool".to_string(),
                node_ref: Some("sart-integration-control-plane".to_string()),
                auto_assign: true,
            },
            status: None,
        }
    }

    pub fn test_address_block_pod_non_default() -> AddressBlock {
        AddressBlock {
            metadata: ObjectMeta {
                finalizers: Some(vec![ADDRESS_BLOCK_FINALIZER.to_string()]),
                name: Some("test-pool-non-default-sart-integration-10.1.0.0".to_string()),
                ..Default::default()
            },
            spec: AddressBlockSpec {
                cidr: "10.1.0.0/24".to_string(),
                r#type: AddressType::Pod,
                pool_ref: "test-pool".to_string(),
                node_ref: None,
                auto_assign: false,
            },
            status: None,
        }
    }

    pub fn test_block_request() -> BlockRequest {
        BlockRequest {
            metadata: ObjectMeta {
                finalizers: Some(vec![BLOCK_REQUEST_FINALIZER.to_string()]),
                name: Some("test-pool-sart-integration-control-plane".to_string()),
                ..Default::default()
            },
            spec: BlockRequestSpec {
                pool: "test-pool".to_string(),
                node: "sart-integration-control-plane".to_string(),
            },
            status: None,
        }
    }

    pub fn test_cluster_bgp() -> ClusterBGP {
        ClusterBGP {
            metadata: ObjectMeta {
                name: Some("test-cb".to_string()),
                finalizers: Some(vec![CLUSTER_BGP_FINALIZER.to_string()]),
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
                    multipath: Some(false),
                },
                peers: Some(vec![PeerConfig {
                    peer_template_ref: Some("test-bgp-peer-templ".to_string()),
                    peer_config: None,
                    node_bgp_selector: None,
                }]),
            },
            status: None,
        }
    }

    pub fn test_bgp_peer_tmpl() -> BGPPeerTemplate {
        BGPPeerTemplate {
            metadata: ObjectMeta {
                name: Some("test-bgp-peer-templ".to_string()),
                ..Default::default()
            },
            spec: BGPPeerTemplateSpec {
                asn: None,
                addr: Some("127.0.0.1".to_string()),
                capabilities: None,
                hold_time: None,
                keepalive_time: None,
                groups: None,
            },
            status: None,
        }
    }

    pub fn test_node_list(selector: TestBgpSelector) -> ObjectList<Node> {
        let list = vec![
            Node {
                metadata: ObjectMeta {
                    name: Some("test1".to_string()),
                    labels: Some(BTreeMap::from([("bgp".to_string(), "a".to_string())])),
                    ..Default::default()
                },
                spec: None,
                status: Some(NodeStatus {
                    addresses: Some(vec![NodeAddress {
                        address: "172.0.0.1".to_string(),
                        type_: "InternalIP".to_string(),
                    }]),
                    ..Default::default()
                }),
            },
            Node {
                metadata: ObjectMeta {
                    name: Some("test2".to_string()),
                    labels: Some(BTreeMap::from([("bgp".to_string(), "a".to_string())])),
                    ..Default::default()
                },
                spec: None,
                status: Some(NodeStatus {
                    addresses: Some(vec![NodeAddress {
                        address: "172.0.0.2".to_string(),
                        type_: "InternalIP".to_string(),
                    }]),
                    ..Default::default()
                }),
            },
            Node {
                metadata: ObjectMeta {
                    name: Some("test3".to_string()),
                    labels: Some(BTreeMap::from([("bgp".to_string(), "b".to_string())])),
                    ..Default::default()
                },
                spec: None,
                status: Some(NodeStatus {
                    addresses: Some(vec![NodeAddress {
                        address: "172.0.0.3".to_string(),
                        type_: "InternalIP".to_string(),
                    }]),
                    ..Default::default()
                }),
            },
        ];
        let filtered = match selector {
            TestBgpSelector::A => vec![list[0].clone(), list[1].clone()],
            TestBgpSelector::B => vec![list[2].clone()],
            TestBgpSelector::All => list,
        };
        ObjectList {
            metadata: ListMeta::default(),
            items: filtered,
        }
    }

    pub fn test_node_bgp_list(selector: TestBgpSelector) -> ObjectList<NodeBGP> {
        let list = vec![
            NodeBGP {
                metadata: ObjectMeta {
                    name: Some("test1".to_string()),
                    labels: Some(BTreeMap::from([("bgp".to_string(), "a".to_string())])),
                    ..Default::default()
                },
                spec: NodeBGPSpec {
                    asn: 65000,
                    router_id: "172.0.0.1".to_string(),
                    speaker: SpeakerConfig {
                        path: "localhost".to_string(),
                        timeout: None,
                        multipath: Some(false),
                    },
                    peers: Some(vec![
                        BGPPeerSlim {
                            name: "test1-peer1".to_string(),
                            spec: BGPPeerSpec {
                                asn: 65000,
                                addr: "172.0.0.1".to_string(),
                                node_bgp_ref: "test1".to_string(),
                                speaker: SpeakerConfig {
                                    path: "localhost".to_string(),
                                    timeout: None,
                                    multipath: Some(false),
                                },
                                groups: None,
                                ..Default::default()
                            },
                        },
                        BGPPeerSlim {
                            name: "test1-peer2".to_string(),
                            spec: BGPPeerSpec {
                                asn: 65000,
                                addr: "172.0.1.1".to_string(),
                                node_bgp_ref: "test1".to_string(),
                                speaker: SpeakerConfig {
                                    path: "localhost".to_string(),
                                    timeout: None,
                                    multipath: Some(false),
                                },
                                groups: None,
                                ..Default::default()
                            },
                        },
                    ]),
                },
                status: None,
            },
            NodeBGP {
                metadata: ObjectMeta {
                    name: Some("test2".to_string()),
                    labels: Some(BTreeMap::from([("bgp".to_string(), "a".to_string())])),
                    ..Default::default()
                },
                spec: NodeBGPSpec {
                    asn: 65000,
                    router_id: "172.0.0.2".to_string(),
                    speaker: SpeakerConfig {
                        path: "localhost".to_string(),
                        timeout: None,
                        multipath: Some(false),
                    },
                    peers: Some(vec![BGPPeerSlim {
                        name: "test2-peer1".to_string(),
                        spec: BGPPeerSpec {
                            asn: 65000,
                            addr: "172.0.0.2".to_string(),
                            node_bgp_ref: "test2".to_string(),
                            speaker: SpeakerConfig {
                                path: "localhost".to_string(),
                                timeout: None,
                                multipath: Some(false),
                            },
                            groups: None,
                            ..Default::default()
                        },
                    }]),
                },
                status: None,
            },
            NodeBGP {
                metadata: ObjectMeta {
                    name: Some("test3".to_string()),
                    labels: Some(BTreeMap::from([("bgp".to_string(), "b".to_string())])),
                    ..Default::default()
                },
                spec: NodeBGPSpec {
                    asn: 65000,
                    router_id: "172.0.0.3".to_string(),
                    speaker: SpeakerConfig {
                        path: "localhost".to_string(),
                        timeout: None,
                        multipath: Some(false),
                    },
                    peers: Some(vec![BGPPeerSlim {
                        name: "test3-peer1".to_string(),
                        spec: BGPPeerSpec {
                            asn: 65000,
                            addr: "172.0.0.3".to_string(),
                            node_bgp_ref: "test3".to_string(),
                            speaker: SpeakerConfig {
                                path: "localhost".to_string(),
                                timeout: None,
                                multipath: Some(false),
                            },
                            groups: None,
                            ..Default::default()
                        },
                    }]),
                },
                status: None,
            },
        ];
        let filtered = match selector {
            TestBgpSelector::A => vec![list[0].clone(), list[1].clone()],
            TestBgpSelector::B => vec![list[2].clone()],
            TestBgpSelector::All => list,
        };
        ObjectList {
            metadata: ListMeta::default(),
            items: filtered,
        }
    }

    pub fn test_bgp_peers() -> ObjectList<BGPPeer> {
        let peers = vec![
            BGPPeer {
                metadata: ObjectMeta {
                    name: Some("test1-peer1".to_string()),
                    ..Default::default()
                },
                spec: BGPPeerSpec {
                    asn: 65000,
                    addr: "172.0.0.1".to_string(),
                    node_bgp_ref: "test1".to_string(),
                    ..Default::default()
                },
                status: None,
            },
            BGPPeer {
                metadata: ObjectMeta {
                    name: Some("test1-peer2".to_string()),
                    ..Default::default()
                },
                spec: BGPPeerSpec {
                    asn: 65000,
                    addr: "172.0.0.1".to_string(),
                    node_bgp_ref: "test1".to_string(),
                    ..Default::default()
                },
                status: None,
            },
            BGPPeer {
                metadata: ObjectMeta {
                    name: Some("test2-peer1".to_string()),
                    ..Default::default()
                },
                spec: BGPPeerSpec {
                    asn: 65000,
                    addr: "172.0.0.2".to_string(),
                    node_bgp_ref: "test2".to_string(),
                    ..Default::default()
                },
                status: None,
            },
            BGPPeer {
                metadata: ObjectMeta {
                    name: Some("test3-peer1".to_string()),
                    ..Default::default()
                },
                spec: BGPPeerSpec {
                    asn: 65000,
                    addr: "172.0.0.3".to_string(),
                    node_bgp_ref: "test3".to_string(),
                    ..Default::default()
                },
                status: None,
            },
        ];
        ObjectList {
            metadata: ListMeta::default(),
            items: peers,
        }
    }

    pub fn test_svc() -> Service {
        test_svc_with_name("test-svc")
    }

    pub fn test_svc_with_name(name: &str) -> Service {
        Service {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some("default".to_string()),
                finalizers: Some(vec![SERVICE_FINALIZER.to_string()]),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                external_traffic_policy: Some("Cluster".to_string()),
                type_: Some("LoadBalancer".to_string()),
                ports: Some(vec![ServicePort {
                    app_protocol: None,
                    name: None,
                    node_port: None,
                    port: 80,
                    protocol: Some("TCP".to_string()),
                    target_port: Some(IntOrString::Int(8888)),
                }]),
                ..Default::default()
            }),
            status: Some(ServiceStatus {
                conditions: None,
                load_balancer: Some(LoadBalancerStatus {
                    ingress: Some(vec![LoadBalancerIngress {
                        ip: Some("10.0.0.1".to_string()),
                        ..Default::default()
                    }]),
                }),
            }),
        }
    }

    pub fn test_eps() -> EndpointSlice {
        EndpointSlice {
            address_type: "IPv4".to_string(),
            endpoints: vec![
                Endpoint {
                    addresses: vec!["192.168.0.1".to_string()],
                    conditions: Some(EndpointConditions {
                        ready: Some(true),
                        serving: Some(true),
                        terminating: Some(false),
                    }),
                    node_name: Some("test1".to_string()),
                    ..Default::default()
                },
                Endpoint {
                    addresses: vec!["192.168.0.2".to_string()],
                    conditions: Some(EndpointConditions {
                        ready: Some(true),
                        serving: Some(true),
                        terminating: Some(false),
                    }),
                    node_name: Some("test2".to_string()),
                    ..Default::default()
                },
                Endpoint {
                    addresses: vec!["192.168.0.2".to_string()],
                    conditions: Some(EndpointConditions {
                        ready: Some(true),
                        serving: Some(true),
                        terminating: Some(false),
                    }),
                    node_name: Some("test1".to_string()),
                    ..Default::default()
                },
                Endpoint {
                    addresses: vec!["192.168.0.2".to_string()],
                    conditions: Some(EndpointConditions {
                        ready: Some(false),
                        serving: Some(false),
                        terminating: Some(true),
                    }),
                    node_name: Some("test1".to_string()),
                    ..Default::default()
                },
            ],
            metadata: ObjectMeta {
                name: Some("test-svc-eps".to_string()),
                namespace: Some("default".to_string()),
                labels: Some(BTreeMap::from([(
                    "kubernetes.io/service-name".to_string(),
                    "test-svc".to_string(),
                )])),
                finalizers: Some(vec![ENDPOINTSLICE_FINALIZER.to_string()]),
                ..Default::default()
            },
            ports: None,
        }
    }

    pub fn test_bgp_advertisement_svc() -> BGPAdvertisement {
        BGPAdvertisement {
            metadata: ObjectMeta {
                name: Some("test-adv".to_string()),
                namespace: Some("default".to_string()),
                finalizers: Some(vec![BGP_ADVERTISEMENT_FINALIZER.to_string()]),
                ..Default::default()
            },
            spec: BGPAdvertisementSpec {
                cidr: "10.0.0.1/32".to_string(),
                r#type: AddressType::Service,
                protocol: Protocol::IPv4,
                attrs: None,
            },
            status: None,
        }
    }
}

pub async fn test_trace() {
    sartd_trace::init::prepare_tracing(sartd_trace::init::TraceConfig {
        level: "info".to_string(),
        format: String::new(),
        file: None,
        _metrics_endpoint: None,
    })
    .await;
}
