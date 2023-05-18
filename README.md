[![GitHub release](https://img.shields.io/github/release/terassyi/sart.svg?maxAge=60)][releases]
![CI](https://github.com/terassyi/sart/workflows/CI/badge.svg)
[![Go Report Card](https://goreportcard.com/badge/github.com/terassyi/sart/controller)](https://goreportcard.com/report/github.com/terassyi/sart/controller)


# Sart

Sart contains BGP speaker implemented in Rust and Kubernetes network load balancer.

> **Warning**
> This project is experimental.

### sartd

Sartd is a simple BGP daemon and Fib manager implementation written in Rust.
Sartd-BGP is based on [RFC 4271](https://datatracker.ietf.org/doc/html/rfc4271).


### sart-controller

Sart-controller is a Kubernetes load balancer like [metallb](https://github.com/metallb/metallb).

## Documentation

- Concept
- Sart
  - [Design](docs/sartd/design.md)
  - [Installation](docs/sartd/install.md)
  - [How to Use](docs/sartd/how_to_use.md)
  - [Quick Start](docs/sartd/quick_start.md)
- Sart Controller
  - [Design](docs/controller/design.md)
  - [Installation](docs/controller/install.md)
  - [How to Use](docs/controller/how_to_use.md)
  - [Quick Start](docs/controller/quick_start.md)

## License

Sart is licensed under the Apache License, Version 2.0. See [LICENSE](https://github.com/terassyi/sart/blob/main/LICENSE) for the full license text.
