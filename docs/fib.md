# How to Use 

Sart has two CLI interfaces, sartd is for daemon and sart-cli is for controlling sartd.

### sartd fib

We can run the Fib manager to install or uninstall routing information in kernel with `sartd fib`.

```console
root@233eff855e37:/# sartd fib -h
Usage: sartd fib [OPTIONS]

Options:
  -e, --endpoint <ENDPOINT>  Fib manager running endpoint url [default: 127.0.0.1:5001]
  -l, --level <LEVEL>        Log level(trace, debug, info, warn, error) [default: info]
  -d, --format <FORMAT>      Log display format [default: plain] [possible values: plain, json]
  -h, --help                 Print help
```
