mod cluster_bgp;
mod node_bgp;
mod bgp_peer_template;
mod bgp_peer;

use kube::CustomResourceExt;

fn main() {
    print!(
        "{}",
        serde_yaml::to_string(&cluster_bgp::ClusterBGP::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&node_bgp::NodeBGP::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&bgp_peer_template::BGPPeerTemplate::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&bgp_peer::BGPPeer::crd()).unwrap()
    );
}
