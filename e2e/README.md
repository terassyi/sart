# End to End Tests

End-to-End tests is written by Golang using [Ginkgo](https://github.com/onsi/ginkgo).
To run this tests, we need Docker and Golang.

Before running tests, we need following command in the `e2e` directory.

```console
$ make setup
```

## BGP

In this test, we prepare testing topologies by [Containerlab](https://containerlab.dev/).

To run the bgp e2e test, please execute the following command.
```console
$ make bgp-e2e
```

We confirm following points in this test.

- Establish peers with other routing softwares.
  - FRR
  - GoBGP
- Establish iBGP and eBGP peers.
- Receive paths from iBGP and eBGP peers.
- Advertise paths to iBGP and eBGP peers.

## Kubernetes

We need to run additional commands to setup a kubernetes cluster and external routes and install Sart related resources.

```console
$ make kubernetes
$ make install-sart
```

After that, we can run the test.

```console
make kubernetes-e2e
```

This is the topology for kubernetes e2e test.

![kubernetes.drawio.svg](./img/kubernetes.drawio.svg)

We confirm that following points in this test.

- Establish BGP peers via Kubernetes custom resources.
- Assign external ip addresses to LoadBalancer services.
  - Specify an address pool.
  - Request a specific address.
  - Specify multiple address pool.
  - Change the address pool.
- Communicate with an external client via LoadBalancer.
  - externalTrafficPolicy=Local
  - externalTrafficPolicy=Cluster
  - Change the externalTrafficPolicy
- Restart
  - sartd-agent
  - sartd-bgp
  - sart-controller
