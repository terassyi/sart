# Quick Start

We can use the e2e test environment for quick start with kind and docker.

To create the environment, run these commands.

```console
$ cd e2e/controller
$ make setup
$ make start
$ make install
```

And we can apply sample manifests.

```console
$ kustomize build manifests | kubectl apply -f -
```

To stop the environment, run this command.

```console
$ make stop
```
