# How to Use

Sart has two CLI interfaces, sartd is for daemon and sart-cli is for controlling sartd.

### sartd bgp

We can run the BGP daemon with `sartd bgp` command.
And we can specify some parameters such as AS number and router id.
To integrate Fib manager, we have to give the path to the gRPC endpoint for fib manager.

```console
Usage: sartd bgp [OPTIONS]

Options:
  -f, --file <FILE>              Config file path for BGP daemon
  -a, --as <AS>                  Local AS Number
  -d, --format <FORMAT>          Log display format [default: plain] [possible values: plain, json]
  -r, --router-id <ROUTER_ID>    Local router id(must be ipv4 format)
      --fib <FIB_ENDPOINT>       Fib endpoint url(gRPC) exp) localhost:5001
      --table-id <FIB_TABLE_ID>  Target fib table id(default is main(254))
      --exporter <EXPORTER>      Exporter endpoint url
  -l, --level <LEVEL>            Log level(trace, debug, info, warn, error) [default: info]
  -h, --help                     Print help
```

We also can configure BGP with a configuration file written by Yaml.
This is the example of config files.

```yaml
asn: 65000
router_id: 10.0.0.2
multi_path: true
neighbors:
  - asn: 65010
    router_id: 10.0.0.3
    address: 10.0.0.3
  - asn: 65020
    router_id: 10.0.1.3
    address: 10.0.1.3
```


## sart Commands

To control running daemons, we can use sart-cli commands.
Now we support following subcommands

- bgp

### sart bgp

`sart bgp` command accepts `global` level and `neighbor` level subcommands.

`global` level can get and set AS number or router id of the local daemon.
And it also can configure RIB(Loc-RIB) information.

```console
root@233eff855e37:/# sart bgp global rib -h
Usage: sart bgp global rib [OPTIONS] <COMMAND>

Commands:
  get
  add
  del
  help  Print this message or the help of the given subcommand(s)

Options:
  -d, --format <FORMAT>      Display format [default: plain] [possible values: plain, json]
  -e, --endpoint <ENDPOINT>  Endpoint to API server [default: localhost:5000]
  -a, --afi <AFI>            Address Family Information [default: ipv4] [possible values: ipv4, ipv6]
  -s, --safi <SAFI>          Sub Address Family Information [default: unicast] [possible values: unicast, multicast]
  -h, --help                 Print help
```

For example, to add a prefix to running BGP daemon, run this command.
```console
root@233eff855e37:/# sart bgp global rib add 10.0.10.0/24 -a ipv4 -t origin=igp
```

`neighbor` level can get and set neighbor information.

**Some commands are not implemented yet.**

```console
root@233eff855e37:/# sart bgp neighbor -h
Usage: sart bgp neighbor [OPTIONS] <COMMAND>

Commands:
  get
  list
  add
  del
  rib
  policy
  help    Print this message or the help of the given subcommand(s)
```

For example, to add a neighbor, run this.

```console
root@233eff855e37:/# sart bgp neighbor add 10.10.0.1 65000
```

## gRPC Interfaces

Sartd has gRPC interfaces.
Sart-cli calls this interface internally.

For detail, please see [proto/](https://github.com/teassyi/sart/blob/main/proto).

## Integrate with FIB Manager

To install paths received from other peers, we need to a FIB manager.
For example, [FRR](https://frrouting.org/) needs to run [zebrad](https://docs.frrouting.org/en/latest/zebra.html) to install paths bgpd gets.

For `sartd-bgp`, we can run `sartd-fib` as the FIB manager.
`Sartd-fib` has gRPC interface to interact with other component.

> [!NOTE]
>`Sartd-fib` is in the PoC phase.

To integrate with `sartd-fib`, we have to specify the gRPC endpoint to FIB manager as a parameter like below.

```console
$ sartd bgp --fib localhost:5010
```
