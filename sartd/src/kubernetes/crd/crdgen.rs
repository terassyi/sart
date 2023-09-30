mod cluster_bgp;

use kube::CustomResourceExt;

fn main() {
    print!(
        "{}",
        serde_yaml::to_string(&cluster_bgp::ClusterBgp::crd()).unwrap()
    );
    println!("---");
}
