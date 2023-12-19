# Design and implementation of sart kubernetes controller

Sart has two components related to Kubernetes.
One is `controller` for controlling [kubernetes custom resources](https://kubernetes.io/docs/concepts/extend-kubernetes/api-extension/custom-resources/) described below.
`controller` is deployed only one in a cluster as `Deployment`. 
Another is `agent` for bridging between its custom resources and BGP speaker running on the node.
`agent` is deployed on each node in a cluster as `DaemonSet`.

And we also need to run `bgp` as `DaemonSet` to work sart kubernetes controller well.

Sart is implemented by pure Rust using [kube-rs](https://github.com/kube-rs/kube).

## Feature

Sart is a Kubernetes load-balancer to announce addresses of [Service type LoadBalancer](https://kubernetes.io/docs/concepts/services-networking/service/#loadbalancer).
[Metallb](https://github.com/metallb/metallb) has a similar feature and sart is inspired by Metallb.

### Allocating addresses for the LoadBalancer

### Announcing LoadBalancer addresses to outside of the cluster

## CRD based architecture

Sart defines some Custom Resource Definitions to represent .
This figure shows how sart's `controller` and `agent` control load balancer addresses and BGP using custom resources.

### CRDs

#### ClusterBGP

ClusterBGP resource controls peering policies in the cluster.
This resource is required one per cluster.

#### NodeBGP

NodeBGP resource exists per nodes.
And this controls a BGP speaker pod on the node such as speaker status and basic information.
This is created by sart-controller when the Node resource is detected.
So we shouldn't create this manually.

#### BGPPeer

BGPPeer resource controls a BGP peer between local speaker on the node and the external speaker.
This has information of the peering state and advertisements.

#### BGPAdvertisement

BGPAdvertisement resource controls the routing information of the load balancer service.
This has the prefix of the announcing address and node names that should advertise its prefix.

This resource should be owned by Service resource.
So we shouldn't create this manually.

#### AddressPool

AddressPool resource is a CIDR for assignable to load balancers.
This resource is the cluster wide resource.
And we can deploy multiple address pools.
