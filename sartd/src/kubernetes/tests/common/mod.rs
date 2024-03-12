use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use kube::core::ObjectMeta;
use sartd_kubernetes::agent::cni::netns::NetNS;
use sartd_kubernetes::crd::{
    address_pool::AddressType,
    bgp_advertisement::{
        AdvertiseStatus, BGPAdvertisement, BGPAdvertisementSpec, BGPAdvertisementStatus, Protocol,
        BGP_ADVERTISEMENT_FINALIZER,
    },
    bgp_peer::{BGPPeer, BGPPeerSlim, BGPPeerSpec, BGP_PEER_FINALIZER},
    cluster_bgp::{SpeakerConfig, ASN_LABEL},
    node_bgp::{NodeBGP, NodeBGPSpec, NodeBGPStatus, NODE_BGP_FINALIZER},
};
use sartd_proto::sart::Args;

// Make sure kind binary is in here
const KIND_BIN: &str = "../../../bin/kind";
const KUBECTL_BIN: &str = "../../../bin/kubectl";
const CRD_MANIFEST: &str = "../../../manifests/base/crd/sart.yaml";
const KIND_CLUSTER_NAME: &str = "sart-integration";
const KIND_CONFIG_DISABLE_CNI: &str = "tests/config/config.yaml";
pub(super) const KIND_NODE_CP: &str = "sart-integration-control-plane";
const KIND_CLUSTER_IMAGE: &str = "kindest/node";
const KIND_CLUSTER_IMAGE_VERSION_ENV: &str = "KIND_NODE_VERSION";

pub fn setup_kind() {
    cleanup_kind_no_output();
    let _config = r#""#;
    let mut binding = std::process::Command::new(KIND_BIN);
    binding.args(["create", "cluster", "--name", KIND_CLUSTER_NAME]);
    if let Ok(v) = std::env::var(KIND_CLUSTER_IMAGE_VERSION_ENV) {
        binding.args(["--image", &format!("{}:{}", KIND_CLUSTER_IMAGE, v)]);
    };

    let out = binding.output().expect("failed to create kind cluster");
    output_result(out);

    let out = std::process::Command::new(KUBECTL_BIN)
        .args(["label", "nodes", "--overwrite", KIND_NODE_CP, "bgp=a"])
        .output()
        .expect("failed to add bgp label");
    output_result(out);

    let out = std::process::Command::new(KUBECTL_BIN)
        .args([
            "label",
            "nodes",
            "--overwrite",
            KIND_NODE_CP,
            &format!("{}=65000", ASN_LABEL),
        ])
        .output()
        .expect("failed to add asn label");
    output_result(out);

    install_crd();

    std::thread::sleep(std::time::Duration::from_secs(2));
}

pub fn cleanup_kind() {
    let out = std::process::Command::new(KIND_BIN)
        .args(["delete", "cluster", "--name", KIND_CLUSTER_NAME])
        .output()
        .expect("failed to delete kind cluster");
    output_result(out);

    std::thread::sleep(std::time::Duration::from_secs(5));
}

pub fn cleanup_kind_no_output() {
    let _ = std::process::Command::new(KIND_BIN)
        .args(["delete", "cluster", "--name", KIND_CLUSTER_NAME])
        .output();
}

fn install_crd() {
    let out = std::process::Command::new(KUBECTL_BIN)
        .args(["apply", "-f", CRD_MANIFEST])
        .output()
        .expect("failed to install crd");
    output_result(out);
}

pub fn kubectl_label(resource: &str, name: &str, label: &str) {
    let out = std::process::Command::new(KUBECTL_BIN)
        .args(["label", resource, "--overwrite", name, label])
        .output()
        .expect("failed to add label");
}

fn output_result(out: std::process::Output) {
    if out.status.success() {
        println!("STDOUT");
        println!("{}", String::from_utf8_lossy(&out.stdout));
    } else {
        println!("STDERR: exit status is {}", out.status);
        println!("{}", String::from_utf8_lossy(&out.stderr));
    }
}

pub fn test_node_bgp() -> NodeBGP {
    NodeBGP {
        metadata: ObjectMeta {
            finalizers: Some(vec![NODE_BGP_FINALIZER.to_string()]),
            name: Some(KIND_NODE_CP.to_string()),
            ..Default::default()
        },
        spec: NodeBGPSpec {
            asn: 65000,
            router_id: "172.0.0.1".to_string(),
            speaker: SpeakerConfig {
                path: "localhost:5000".to_string(),
                timeout: None,
                multipath: Some(false),
            },
            peers: Some(vec![BGPPeerSlim {
                name: "test-peer1".to_string(),
                spec: BGPPeerSpec::default(),
            }]),
        },
        status: Some(NodeBGPStatus {
            backoff: 0,
            ..Default::default()
        }),
    }
}

pub fn test_bgp_peer() -> BGPPeer {
    BGPPeer {
        metadata: ObjectMeta {
            name: Some("test-peer1".to_string()),
            finalizers: Some(vec![BGP_PEER_FINALIZER.to_string()]),
            ..Default::default()
        },
        spec: BGPPeerSpec {
            asn: 65000,
            addr: "172.0.0.1".to_string(),
            node_bgp_ref: KIND_NODE_CP.to_string(),
            speaker: SpeakerConfig {
                path: "localhost:5000".to_string(),
                timeout: None,
                multipath: Some(false),
            },
            ..Default::default()
        },
        status: None,
    }
}

pub fn test_bgp_advertisement() -> BGPAdvertisement {
    BGPAdvertisement {
        metadata: ObjectMeta {
            name: Some("test-adv1".to_string()),
            namespace: Some("default".to_string()),
            finalizers: Some(vec![BGP_ADVERTISEMENT_FINALIZER.to_string()]),
            ..Default::default()
        },
        spec: BGPAdvertisementSpec {
            cidr: "10.0.0.0/32".to_string(),
            r#type: AddressType::Service,
            protocol: Protocol::IPv4,
            attrs: None,
        },
        status: Some(BGPAdvertisementStatus {
            peers: Some(BTreeMap::from([
                ("test-peer1".to_string(), AdvertiseStatus::NotAdvertised),
                ("test-peer2".to_string(), AdvertiseStatus::NotAdvertised),
            ])),
        }),
    }
}

pub struct TestContainer {
    pub args: Args,
    pub netns: NetNS,
}

impl TestContainer {
    pub fn new(
        container_id: &str,
        netns: &str,
        ifname: &str,
        path: &str,
        uid: &str,
        pod_name: &str,
        namespace: &str,
    ) -> Self {
        let ns_path = PathBuf::from_str(netns).unwrap();
        let mut cmd = std::process::Command::new("ip");
        cmd.args([
            "netns",
            "add",
            ns_path.file_name().unwrap().to_str().unwrap(),
        ]);
        cmd.output().unwrap();

        TestContainer {
            args: Args {
                container_id: container_id.to_string(),
                netns: netns.to_string(),
                ifname: ifname.to_string(),
                path: vec![path.to_string()],
                // K8S_POD_INFRA_CONTAINER_ID=0a6a4b09df59d64e3be5cf662808076fee664447a1c90dd05a5d5588e2cd6b5a;K8S_POD_UID=b0e1fc4a-f842-4ec2-8e23-8c0c8da7b5e5;IgnoreUnknown=1;K8S_POD_NAMESPACE=kube-system;K8S_POD_NAME=coredns-787d4945fb-7xrrd
                args: format!("K8S_POD_INFRA_CONTAINER_ID={container_id};K8S_POD_UID={uid};IgnoreUnknown=1;K8S_POD_NAMESPACE={namespace};K8S_POD_NAME={pod_name}"),
                prev_result: None,
                conf: None,
                data: String::new(),
            },
            netns: NetNS::try_from(netns).unwrap(),
        }
    }
}

impl Drop for TestContainer {
    fn drop(&mut self) {
        let mut cmd = std::process::Command::new("ip");
        cmd.args([
            "netns",
            "del",
            self.netns.path().file_name().unwrap().to_str().unwrap(),
        ]);
        cmd.output().unwrap();
    }
}

pub struct TestRoutingRule {
    v6: bool,
    table: u32,
}

impl TestRoutingRule {
    pub fn new(table: u32, v6: bool) -> Self {
        Self { v6, table }
    }
}

impl Drop for TestRoutingRule {
    fn drop(&mut self) {
        let mut cmd = std::process::Command::new("ip");
        if self.v6 {
            cmd.arg("-6");
        }
        cmd.args(["rule", "del", "table", &format!("{}", self.table)]);
        cmd.output().unwrap();
    }
}
