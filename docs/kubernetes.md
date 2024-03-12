# How to Use as Kubernetes network load-balancer

To use kubernetes network load-balancer, we need to run three components: `sartd controller`, `sartd agent` and `sartd bgp`.

The `controller` runs as `Deployment`.
And the `agent` and `bgp` run as `DaemonSet`.
`agent` and `bgp` must run in host network namespace.

To know sart's CRDs, please see [design #Kubernetes](./design.md#kubernetes)

## Address pools

`AddressPool` is a cluster-scope resource.

LoadBalancer services are assigned IP addresses from an address pool.
And pods is also assigned their IP address from this.

The `type` field specifies for which use the address pool is to be used.

This is an example address pool for LoadBalancer.
To use for LoadBalancer, we have to set `type: service`.

```yaml
apiVersion: sart.terassyi.net/v1alpha2
kind: AddressPool
metadata:
  name: default-lb-pool
spec:
  cidr: 10.0.1.0/24
  type: service
  allocType: bit
  blockSize: 24
  autoAssign: true
```

`cidr` is the subnet of assignable addresses.
Now, we support only IPv4.

`allocType` specifies a method how to pick an address from a pool.
We can only use `bit`.

Bit allocator type choose the address with the lowest number.
For example, `10.0.0.1` and `10.0.0.10` are assignable, the bit allocator always chooses `10.0.0.1`.

`blockSize` is used to divide the address pool into address blocks.
In case of `type: service`, `blockSize` must be equal to a cidr's prefix length.

`autoAssign` specifies its pool is used as default pool.
If `autoAssign` is true, we can omit specifying the name of the pool in the annotation(described below).
We cannot create multiple auto assignable pools in each `type`.

Please note that we cannot change `AddressPool`'s spec fields except for `autoAssign` once it is created.

## BGP configurations

To announce assigned addresses to external network, Sart use BGP.

Before configuring BGP, we have to confirm that `sartd-bgp` runs well.

### ClusterBGP

To configure the cluster wide BGP configurations, we can create `ClusterBGP` resources.
`ClusterBGP` is a cluster-scope resource.

`ClusterBGP` resource templates local BGP speaker settings and peer information for each node.

This is an example of `ClusterBGP`.

```yaml
apiVersion: sart.terassyi.net/v1alpha2
kind: ClusterBGP
metadata:
  name: clusterbgp-sample-a
spec:
  nodeSelector:
    bgp: a
  asnSelector:
    from: label
  routerIdSelector:
    from: internalAddress
  speaker:
    path: 127.0.0.1:5000
  peers:
    - peerTemplateRef: bgppeertemplate-sample
      nodeBGPSelector:
        bgp: a
```

If `nodeSelector` is specified, `ClusterBGP` creates `NodeBGP` resources on only nodes that has labels matching its selector.
If `nodeSelector` is not specified, this creates `NodeBGP` resources on all nodes.
a `NodeBGP` resource is created with the same name as the node.


`asnSelector` and `routerIdSelector` is used to configure `NodeBGP` resources.

`asnSelector.from` can have `asn` or `label`.
If `asnSelector.from` is `asn`, `asnSelector.asn` must be set as valid ASN value.
If it is `label`, ASN value is got from the label named `sart.terassyi.net/asn` added to the node.

`routerIdSelector.from` can have `internalAddress`, `label` and `routerId`.
In case of `internalAddress`, `NodeBGP`'s router id is got from the node resource's address.
When `label` is specified, router id is got from the label named `sart.terassyi.net/router-id` on the node.
And when `routerId` is specified, router id is got from `routerIdSelector.routerId`.

`speaker` specifies the endpoint to the local BGP speaker on the node.
When `NodeBGP` resource is created and local speaker is configured on the node, `agent` on the same node interacts with the local BGP speaker via its endpoint.

`peers` can have the array of `BGPPeerTemplate`(described details in the following section).
We can specify a name of existing `BGPPeerTemplate` resource in a `peerTemplateRef`.
And we can also specify a spec field of `BGPPeerTemplate` inline.
We can choose target node with `nodeBGPSelector`.

### BGPPeerTemplate

We can define `BGPPeerTemplate` as a template of BGP peers.
If a `ClusterBGP` resource is created and `peerTemplateRef` is specified, `BGPPeer` resources are created based on this template.

```yaml
apiVersion: sart.terassyi.net/v1alpha2
kind: BGPPeerTemplate
metadata:
  name: bgppeertemplate-sample
spec:
  asn: 65000
  addr: 9.9.9.9
  groups:
    - to-router0
```

### BGPPeer

`BGPPeer` represents BGP peer in the local BGP speaker.
`BGPPeer` is a cluster-scope resource.

This is an example.

```yaml
apiVersion: sart.terassyi.net/v1alpha2
kind: BGPPeer
metadata:
  labels:
    bgp: b
    bgppeer.sart.terassyi.net/node: sart-control-plane
  name: bgppeer-sample-sart-cp
spec:
  addr: 9.9.9.9
  asn: 65000
  groups:
    - to-router0
  nodeBGPRef: sart-control-plane
  speaker:
    path: 127.0.0.1:5000
```

`asn` and `addr` specifies the remote BGP speaker configuration.
`speaker` is for the endpoint to local BGP speaker.

`BGPPeer` is related to the specific `NodeBGP`.
So we need to give the node name in `nodeBGPRef`.

`BGPPeer`'s `status` field represents the actual state of BGP peer and we can check the status by `kubectl get bgppeer` like below.

```console
$ kubectl get bgppeer
NAME                                                ASN     ADDRESS   NODEBGP              STATUS        AGE
bgppeer-sample-sart-cp                              65000   9.9.9.9   sart-control-plane   Established   8s
bgppeertemplate-sample-sart-worker-65000-9.9.9.9    65000   9.9.9.9   sart-worker          Established   2s
bgppeertemplate-sample-sart-worker2-65000-9.9.9.9   65000   9.9.9.9   sart-worker2         Established   2s
bgppeertemplate-sample-sart-worker3-65000-9.9.9.9   65000   9.9.9.9   sart-worker3         OpenConfirm   1s
```

## LoadBalancer Service

If we complete to prepare BGP related resources and `AddressPools`, we can use LoadBalancer services.

After adding pools to the cluster, it appears like so.
Sample manifests are [here](../manifests/sample/lb_address_pool.yaml).

```console
$ kubectl get addresspool                           
NAME                  CIDR            TYPE      BLOCKSIZE   AUTO    AGE
default-lb-pool       10.0.1.0/24     service   24          true    13s
non-default-lb-pool   10.0.100.0/24   service   24          false   13s
```

### Basic usage

A basic Service type LoadBalancer looks like this.

```yaml
apiVersion: v1
kind: Service
metadata:
  name: app-svc-cluster
  namespace: test
  annotations:
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

After creating this, we can confirm that `EXTERNAL-IP` is assigned from the `default-lb-pool`.
In this case, there isn't requested pools. 
So the LoadBalancer address is assigned from the `default-lb-pool`.

```console
$ kubectl -n test get svc app-svc-cluster       
NAME              TYPE           CLUSTER-IP       EXTERNAL-IP   PORT(S)        AGE
app-svc-cluster   LoadBalancer   10.101.205.218   10.0.1.0      80:31012/TCP   11m
```

### Choosing AddressPool

LoadBalancer services can choose address pools.
To specify it, we use the annotation named `sart.terassyi.net/addresspool` like this.

```yaml
apiVersion: v1
kind: Service
metadata:
  name: app-svc-cluster2
  namespace: test
  annotations:
    sart.terassyi.net/addresspool: non-default-lb-pool
spec:
  type: LoadBalancer
  externalTrafficPolicy: Cluster
  selector:
    app: app-cluster2
  ports:
    - name: http
      port: 80
      targetPort: 80
```

And we can confirm that the `non-default-lb-pool` is used.

```console
$ kubectl -n test get svc app-svc-local       
NAME            TYPE           CLUSTER-IP      EXTERNAL-IP   PORT(S)        AGE
app-svc-local   LoadBalancer   10.101.47.224   10.0.100.0    80:31291/TCP   34s
```

### Requesting addresses

LoadBalancer services can also request specific addresses.
To request it, we use the annotation named `sart.terassyi.net/loadBalancerIPs` like this.

In this example, this service requests `10.0.100.20` from the `non-default-lb-pool`.

```yaml
apiVersion: v1
kind: Service
metadata:
  name: app-svc-cluster2
  namespace: test
  annotations:
    sart.terassyi.net/addresspool: non-default-lb-pool
    sart.terassyi.net/loadBalancerIPs: "10.0.100.20"
spec:
  type: LoadBalancer
  externalTrafficPolicy: Cluster
  selector:
    app: app-cluster2
  ports:
    - name: http
      port: 80
      targetPort: 80
```

```console
$ kubectl -n test get svc app-svc-cluster2       
NAME               TYPE           CLUSTER-IP      EXTERNAL-IP   PORT(S)        AGE
app-svc-cluster2   LoadBalancer   10.101.186.67   10.0.100.20   80:31857/TCP   17m
```

### Assigning multiple addresses

We can assign multiple addresses to one LoadBalancer from multiple pools.

This manifest requests to assign multiple addresses from `default-lb-pool` and `non-default-lb-pool`.
To specify multiple value in the annotation, we have to specify values separated by commas.
And it also requests `10.0.100.22`.

```yaml
apiVersion: v1
kind: Service
metadata:
  name: app-svc-cluster3
  namespace: test
  annotations:
    sart.terassyi.net/addresspool: default-lb-pool,non-default-lb-pool
    sart.terassyi.net/loadBalancerIPs: "10.0.100.22"
spec:
  type: LoadBalancer
  externalTrafficPolicy: Cluster
  selector:
    app: app-cluster3
  ports:
    - name: http
      port: 80
      targetPort: 80
```

In this case, `default-lb-pool` assigns an address automatically and `non-default-lb-pool` assigns the specified address(`10.0.100.22`).


```console
$ kubectl -n test get svc app-svc-cluster3
NAME               TYPE           CLUSTER-IP      EXTERNAL-IP            PORT(S)        AGE
app-svc-cluster3   LoadBalancer   10.101.38.202   10.0.1.1,10.0.100.22   80:31980/TCP   5m45s
```

When we choose multiple pools and even if one of these pool is auto assignable, we cannot omit specifying its name in the annotation.

We also can request multiple addresses from one pool.
