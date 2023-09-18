
pub(crate) mod reconcilers;
pub(crate) mod context;
pub(crate) mod error;
pub(crate) mod metrics;
pub(crate) mod bgp;

use kube::CustomResourceExt;


fn main() {
    print!("{}", serde_yaml::to_string(&crate::reconcilers::clusterbgp::ClusterBgp::crd()).unwrap());
    println!("---");
    print!("{}", serde_yaml::to_string(&reconcilers::bgppeer::BgpPeer::crd()).unwrap());
    println!("---");
    print!("{}", serde_yaml::to_string(&reconcilers::bgpadvertisement::BgpAdvertisement::crd()).unwrap());
    println!("---");
    print!("{}", serde_yaml::to_string(&reconcilers::addresspool::AddressPool::crd()).unwrap());
}
