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
  - Configuration
  - Quick Start
- Sart Controller
  - [Design](docs/controller/design.md)
  - [Installation](docs/controller/install.md)
  - Custom Resource Definition
  - Configuration
  - Quick Start
