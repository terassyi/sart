# Design of sart-controller

Sart-controller is a self made Kubernetes network load balancer for announcing [externalIP of Service](https://kubernetes.io/docs/concepts/services-networking/service/#loadbalancer).

[metallb](https://github.com/metallb/metallb) has a similar feature.

To announce load balancer IP addresses to the external system, sart-controller uses BGP.
So we need BGP speakers outside the Kubernetes cluster and also BGP speaker inside the cluster.
Local(inside) BGP speakers are on the each nodes as DaemonSet.
And sart-controller controls these speakers via API.
Now, we can only use sartd-bgp as local speakers.

> **Note**
> Sart-controller is an alpha system.

## CRD based architecture

This figure is a work flow of sart-controller.

Sart-controller is a Kubernetes custom controller powered by [Kubebuilder](https://github.com/kubernetes-sigs/kubebuilder).
And it controls BGP speakers, BGP sessions and routing information of the load balancer addresses as Custom Resource Definitions.

![model](model.drawio.svg)

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

## Assign addresses

We can assign load balancer addresses to LoadBalancer services from address pools by using the `sart.terassyi.net/address-pool` annotation as shown bellow.

When sart-controller find the service resource with this annotation, it assign the address from the specified address pool and create and advertise routing information.

```yaml
apiVersion: v1
kind: Service
metadata:
  name: app-svc
  namespace: default
  annotations:
    sart.terassyi.net/address-pool: default
spec:
  type: LoadBalancer
  externalTrafficPolicy: Cluster
  selector:
    app: app-cluster
  ports:
    - name: http
      port: 80
      targetPort: 80
```
