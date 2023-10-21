mod address_pool;
mod bgp_advertisement;
mod bgp_peer;
mod bgp_peer_template;
mod cluster_bgp;
mod error;
mod node_bgp;

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
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&bgp_advertisement::BGPAdvertisement::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&address_pool::AddressPool::crd()).unwrap()
    );
}
