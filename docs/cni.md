# How to use as Kubernetes CNI plugin

To run as Kubernetes CNI plugin, we need to run four components: `sartd controller`, `sartd agent`, `sartd bgp` and `sartd fib`.

`controller` runs as `Deployment`.
And `agent`, `bgp` and `fib` run as `DaemonSet`.

To perform as CNI plugin, all components must run in host network namespace.

To know sart's CRDs, please see [design #Kubernetes](./design.md#kubernetes) and [design #CNI](./design.md#cni-for-kubernetes).

To work CNI well, we need to configure BGP speakers on nodes.
Please see [Kubernetes BGP configurations](./kubernetes.md#bgp-configurations).

## Address pools


To assign IP addresses to pods, we have to create address pools.
We can create multiple pools in a cluster.

The `type` field specifies for which use the address pool is to be used.
For pods, we have to set `type: pod`.

```yaml
apiVersion: sart.terassyi.net/v1alpha2
kind: AddressPool
metadata:
  name: default-pod-pool
spec:
  cidr: 10.1.0.0/24
  type: pod
  allocType: bit
  blockSize: 29
  autoAssign: true
```

`cidr` is the subnet of assignable addresses for pods.

`allocType` specifies a method how to pick an address from a pool.
We can only use `bit`.

Bit allocator type choose the address with the lowest number.
For example, `10.0.0.1` and `10.0.0.10` are assignable, the bit allocator always chooses `10.0.0.1`.

`blockSize` is used to divide the address pool into `AddressBlock`s.
`AddressBlock` for pods is associated with a node and it is requested to create via a `BlockRequest` resource.
Each block requests its cidr based on this value.

In this example, the cidr of one address block is `10.1.0.0/29` and another is `10.1.0.8/29`.

`autoAssign` specifies its pool is used as default pool.
If `autoAssign` is true, we can omit specifying the name of the pool in the annotation(described below).
We cannot create multiple auto assignable pools in each `type`.

Please note that we cannot change `AddressPool`'s spec fields except for `autoAssign` once it is created.

### Basic usage

Once creating `AddressPool`s, we can create some pods same as using other CNI plugins.
When allocating from the auto assignable pool, there is nothing to do any other special procedure.

### Choosing AddressPool

Sart CNI can create multiple pools for pods.
To use the pool that is not auto assignable pool, we should add an annotation named `sart.terassyi.net/addresspool` to the namespace in which its pod exist or to the pod directly.

