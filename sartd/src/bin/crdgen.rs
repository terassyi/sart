use kube::CustomResourceExt;
use sartd_kubernetes::crd;

fn main() {
    print!(
        "{}",
        serde_yaml::to_string(&crd::cluster_bgp::ClusterBGP::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&crd::node_bgp::NodeBGP::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&crd::bgp_peer_template::BGPPeerTemplate::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&crd::bgp_peer::BGPPeer::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&crd::bgp_advertisement::BGPAdvertisement::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&crd::address_pool::AddressPool::crd()).unwrap()
    );
    println!("---");
    print!(
        "{}",
        serde_yaml::to_string(&crd::address_block::AddressBlock::crd()).unwrap()
    );
}
