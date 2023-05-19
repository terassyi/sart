# Installation of sart-controller

## Use released manifests

To install sart-controller, apply the manifest.

> **Note**
> Before applying sart-controller manifests, we have to deploy cert-manager for webhooks.

```console
$ kubectl apply -f https://github.com/terassyi/sart/releases/download/v<VERSION>/sart-controller.yaml
```

## Build with kustomize

We also can install the manifest by building with kustomize.

```console
$ kustomize build controller/config/release | kubectl apply -f -
```
