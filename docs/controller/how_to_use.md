# How to Use

Before using sart-controller, [install](install.md) it.

First, we add the label that the key is `sart.terassyi.net/asn` and the value is ASN of the local BGP speaker for each nodes.

```console
kubectl label nodes --overwrite <NODE NAME> sart.terassyi.net/asn=<ASN>
```

Next, we need to create a `ClusterBGP` resource for each kubernetes cluster.
We can apply the manifest shown bellow.

```yaml
apiVersion: sart.terassyi.net/v1alpha1
kind: ClusterBGP
metadata:
  name: clusterbgp-sample
  namespace: kube-system
spec:
  peeringPolicy:
    policy: none
```

And, we have to create BGP peers between external BGP speakers and each local speakers on each nodes.
We have to create a `BGPPeer` resource for each BGP peer like bellow.

In this manifest, we need to satisfy `spec.node` or `spec.peerAsn` and `spec.peerRouterId` to identify the BGP peer.

```yaml
apiVersion: sart.terassyi.net/v1alpha1
kind: BGPPeer
metadata:
  name: bgppeer-sample
  namespace: kube-system
spec:
  node: node-sample
  peerAsn: 65000
  peerRouterId: 172.18.0.2
```

Finally, we create `AddressPool` resources.
`spec.type` must be `lb`.
And we can specify the routable CIDR to `spec.prefix`.

```yaml
apiVersion: sart.terassyi.net/v1alpha1
kind: AddressPool
metadata:
  name: sample-pool
spec:
  type: lb
  cidrs:
    - protocol: ipv4
      prefix: 10.69.0.0/24
```

Now, we can create `Service(type LoadBalancer)` resources.
We can use the address pool by applied above by annotating `sart.terassyi.net/address-pool: sample-pool`.

This is an example.

```yaml
apiVersion: v1
kind: Service
metadata:
  name: app-svc
  namespace: test
  annotations:
    sart.terassyi.net/address-pool: sample-pool
spec:
  type: LoadBalancer
  externalTrafficPolicy: Cluster
  selector:
    app: app
  ports:
    - name: http
      port: 80
      targetPort: 80
```
