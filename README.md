![GitHub release](https://img.shields.io/github/release/terassyi/sart.svg?maxAge=60)
![CI](https://github.com/terassyi/sart/workflows/ci/badge.svg)


# Sart

Sart is the Kubernetes network load-balancer and CNI plugin for Kubernetes using BGP in Rust.

This project is inspired by [Metallb](https://github.com/metallb/metallb) and [Coil](https://github.com/cybozu-go/coil).

> [!WARNING]
> This project is under experimental.
> CNI feature is not implemented yet.

## Features

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

## Quick Start

Sart can run on the container based environment using [kind](https://kind.sigs.k8s.io/) and [containerlab](https://containerlab.dev/).

And we also need to [install Rust and Cargo](https://doc.rust-lang.org/cargo/getting-started/installation.html).

First, we have to create the test environment.
```console
$ make build-image
$ make certs
$ make crd
$ cd e2e
$ make setup
$ make kubernetes
$ make install-sart
$ make sample # create sample workloads
```

After that, we can confirm `EXTENAL-IPs` are assigned and the connectivity.

```console
$ kubectl -n test get svc
NAME               TYPE           CLUSTER-IP       EXTERNAL-IP   PORT(S)        AGE
app-svc-cluster    LoadBalancer   10.101.96.120    10.0.1.0      80:30870/TCP   2m48s
app-svc-cluster2   LoadBalancer   10.101.153.185   10.0.100.20   80:32601/TCP   2m48s
app-svc-local      LoadBalancer   10.101.140.238   10.0.1.1      80:31642/TCP   2m48s
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
