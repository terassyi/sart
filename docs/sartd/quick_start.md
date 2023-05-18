# Quick Start

We can create some topology for quick start of sartd-bgp.

To create the topology, run these commands.

```console
$ cd e2e/sartd
$ go run main.go list
# please specify topology name got by list command(in this example, use simple-with-zebra).
$ go run main.go build simple-with-zebra
```

To clean up the topology, run this command.

```console
$ go run main.go clean simple-with-zebra
```
