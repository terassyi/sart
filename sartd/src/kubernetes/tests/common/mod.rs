use std::collections::BTreeMap;

use kube::core::ObjectMeta;
use sartd_kubernetes::crd::{
    address_pool::AddressType,
    bgp_advertisement::{
        AdvertiseStatus, BGPAdvertisement, BGPAdvertisementSpec, BGPAdvertisementStatus, Protocol,
        BGP_ADVERTISEMENT_FINALIZER,
    },
    bgp_peer::{BGPPeer, BGPPeerSlim, BGPPeerSpec, BGP_PEER_FINALIZER},
    cluster_bgp::{SpeakerConfig, ASN_LABEL},
    node_bgp::{NodeBGP, NodeBGPSpec, NODE_BGP_FINALIZER},
};

pub fn hoge() {
    println!("hoge")
}

// Make sure kind binary is in here
const KIND_BIN: &str = "../../../bin/kind";
const KUBECTL_BIN: &str = "../../../bin/kubectl";
const CRD_MANIFEST: &str = "../../../manifests/crd/sart.yaml";
const KIND_CLUSTER_NAME: &str = "sart-integration";
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

    std::thread::sleep(std::time::Duration::from_secs(2));
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
            },
            peers: Some(vec![BGPPeerSlim {
                name: "test-peer1".to_string(),
                spec: BGPPeerSpec::default(),
            }]),
        },
        status: None,
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
