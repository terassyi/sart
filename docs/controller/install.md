# Installation of sart-controller

## Use released manifests

To install sart-controller, apply the manifest.

```console
$ kubectl apply -f https://github.com/terassyi/sart/releases...
```

## Build with kustomize

We also can install the manifest by building with kustomize.

```console
$ kustomize build controller/config/release | kubectl apply -f -
```
