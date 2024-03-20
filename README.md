![GitHub release](https://img.shields.io/github/release/terassyi/sart.svg?maxAge=60)
![CI](https://github.com/terassyi/sart/workflows/ci/badge.svg)


# Sart

Sart is the Kubernetes network load-balancer and CNI plugin for Kubernetes using BGP in pure Rust.

This project is inspired by [Metallb](https://github.com/metallb/metallb) and [Coil](https://github.com/cybozu-go/coil).

> [!WARNING]
> This project is under experimental.

## Features

### CNI

Sart has a CNI plugin feature to configure pod network settings.

Now sart supports only an IPAM feature.

### Assigning an address to Pods

To assign an IP address to a pod and configure the network settings such as routing information, we have to create `AddressPool` resources.
Sart can handle multiple AddressPool.
So we can select the pool to use per a pod or a namespace.
To specify the pool, we can add an annotation `sart.terassyi.net/addresspool` to a pod or a namespace.

For more details, please see [docs/cni.md](./docs/cni.md).

### Quick Start

Sart can run on the container based environment using [kind](https://kind.sigs.k8s.io/) and [containerlab](https://containerlab.dev/).

And we also need to [install Rust and Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html).

First, we have to create the test environment and run e2e tests.

For more information about e2e tests, please see [e2e/README.md](./e2e/README.md)

```console
$ make build-image
$ make certs
$ make crd
$ cd e2e
$ make setup
$ make kubernetes MODE=cni
$ make install-sart MODE=cni
$ make kubernetes-e2e MODE=cni
```

After running e2e test, we can find pods are running.

```console
$ kubectl get pod -n test -owide
NAME                      READY   STATUS    RESTARTS   AGE   IP          NODE                 NOMINATED NODE   READINESS GATES
test-cp                   1/1     Running   0          11h   10.1.0.0    sart-control-plane   <none>           <none>
test-worker               1/1     Running   0          11h   10.1.0.8    sart-worker          <none>           <none>
test-worker-non-default   1/1     Running   0          11h   10.10.0.0   sart-worker          <none>           <none>
test-worker2              1/1     Running   0          11h   10.1.0.24   sart-worker2         <none>           <none>
test-worker3              1/1     Running   0          11h   10.1.0.16   sart-worker3         <none>           <none>
test-worker3-2            1/1     Running   0          11h   10.1.0.17   sart-worker3         <none>           <none>
test-worker3-3            1/1     Running   0          11h   10.1.0.18   sart-worker3         <none>           <none>
```

And we can confirm that each pod has the reachability to the other pod.

```console
$ kubectl -n test exec -it test-cp -- ping -c 1 10.1.0.17
PING 10.1.0.17 (10.1.0.17) 56(84) bytes of data.
64 bytes from 10.1.0.17: icmp_seq=1 ttl=61 time=0.155 ms

--- 10.1.0.17 ping statistics ---
1 packets transmitted, 1 received, 0% packet loss, time 0ms
rtt min/avg/max/mdev = 0.155/0.155/0.155/0.000 ms
```

To clean up, run `make kubernetes-down MODE=cni`.


## Load Balancer

### Allocating LoadBalancer addresses

Sart can create multiple AddressPools to define the range of IP addresses usable for Kubernetes service type LoadBalancer.

Sart assigns addresses picked from created AddressPools to LoadBaldncer automatically.
And we can specify the pool to allocate with the annotation `sart.terassyi.net/addresspool`.

We can also control which addresses we allocate to the LoadBalancer with the annotation `sart.terassyi.net/loadBalancerIPs`.
Moreover, we can assign multiple addresses(even if these belong to different pools) to one LoadBalancer.

### Exporting LoadBalancer addresses using BGP

To work on this, we need BGP speakers on each node.
Sart implements the BGP speaker feature and provides its abstraction layer as Kubernetes Custom Resources.

Please see detail manifests in [manifests/sample](manifests/sample/).

### Quick Start

First, we have to create the test environment and run e2e tests.

For more information about e2e tests, please see [e2e/README.md](./e2e/README.md)

```console
$ make build-image
$ make certs
$ make crd
$ cd e2e
$ make setup
$ make kubernetes
$ make install-sart
$ make kubernetes-e2e
```

After that, we can confirm `EXTENAL-IPs` are assigned and the connectivity.

```console
$ kubectl -n test get svc
NAME               TYPE           CLUSTER-IP       EXTERNAL-IP           PORT(S)        AGE
app-svc-cluster    LoadBalancer   10.101.239.94    10.0.1.0,10.0.100.0   80:31840/TCP   39m
app-svc-cluster2   LoadBalancer   10.101.200.19    10.0.100.20           80:31421/TCP   39m
app-svc-cluster3   LoadBalancer   10.101.93.36     10.0.1.2              80:32415/TCP   36m
app-svc-local      LoadBalancer   10.101.201.125   10.0.1.1              80:30250/TCP   39m
app-svc-local2     LoadBalancer   10.101.21.50     10.0.100.1            80:32374/TCP   36m
```

```console
$ docker exec -it clab-sart-client0 curl http://10.0.1.0
<!DOCTYPE html>
<html>
<head>
<title>Welcome to nginx!</title>
...
```

## License

Sart is licensed under the Apache License, Version 2.0. See [LICENSE](https://github.com/terassyi/sart/blob/main/LICENSE) for the full license text.
