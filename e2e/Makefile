include Makefile.versions

SUDO ?= sudo

BINDIR := $(abspath $(PWD)/bin)
MANIFESTDIR := $(abspath $(PWD)/../manifests/base)
LB_MANIFESTDIR := $(abspath $(PWD)/../manifests/lb)
CNI_MANIFESTDIR := $(abspath $(PWD)/../manifests/cni)
TOPOLOGYDIR := $(abspath $(PWD)/topology)

KIND := $(BINDIR)/kind
KUBECTL := $(BINDIR)/kubectl
KUSTOMIZE := $(BINDIR)/kustomize
CONTAINERLAB := $(BINDIR)/containerlab
HELM := $(BINDIR)/helm

IMAGE_NAME := sart
IMAGE_VERSION := dev
CLUSTER_NAME = sart
KIND_CONFIG = kind-config.yaml
CNI_KIND_CONFIG = kind-config-disable-cni.yaml
CONTAINERLAB_KUBERNETES = $(TOPOLOGYDIR)/kubernetes.yaml

MODE := lb

WORKER_ASN ?= 65000
WORKER2_ASN ?= 65000
WORKER3_ASN ?= 65000
CP_ASN ?= 65000

FEATURE =
.PHONY: bgp-e2e
bgp-e2e:
	go test -c ./...
	$(SUDO) TARGET=bgp ./e2e.test --ginkgo.v -test.v
	rm -f ./e2e.test

.PHONY: kubernetes-e2e
kubernetes-e2e:
	go test -c ./...
	TARGET=kubernetes ./e2e.test --ginkgo.v -test.v
	rm -f ./e2e.test

.PHONY: topology
topology:

.PHONY: kubernetes
kubernetes:
	$(SUDO) sysctl -w fs.inotify.max_user_instances=512
	$(SUDO) sysctl -w fs.inotify.max_user_watches=65536
ifeq ($(MODE),cni)
	$(KIND) create cluster --image kindest/node:v$(KUBERNETES_VERSION) --config=$(CNI_KIND_CONFIG) --name $(CLUSTER_NAME)
else
	$(KIND) create cluster --image kindest/node:v$(KUBERNETES_VERSION) --config=$(KIND_CONFIG) --name $(CLUSTER_NAME)
endif
	$(SUDO) $(CONTAINERLAB) -t $(CONTAINERLAB_KUBERNETES) deploy

	$(KUBECTL) label nodes --overwrite $(CLUSTER_NAME)-worker bgp=a
	$(KUBECTL) label nodes --overwrite $(CLUSTER_NAME)-worker2 bgp=a
	$(KUBECTL) label nodes --overwrite $(CLUSTER_NAME)-worker3 bgp=a
	$(KUBECTL) label nodes --overwrite $(CLUSTER_NAME)-control-plane bgp=b
	$(KUBECTL) label nodes --overwrite $(CLUSTER_NAME)-worker sart.terassyi.net/asn=$(WORKER_ASN)
	$(KUBECTL) label nodes --overwrite $(CLUSTER_NAME)-worker2 sart.terassyi.net/asn=$(WORKER2_ASN)
	$(KUBECTL) label nodes --overwrite $(CLUSTER_NAME)-worker3 sart.terassyi.net/asn=$(WORKER3_ASN)
	$(KUBECTL) label nodes --overwrite $(CLUSTER_NAME)-control-plane sart.terassyi.net/asn=$(CP_ASN)

	$(KIND) load docker-image $(IMAGE_NAME):$(IMAGE_VERSION) -n $(CLUSTER_NAME)

.PHONY: kubernetes-down
kubernetes-down:
	$(SUDO) $(CONTAINERLAB) -t $(CONTAINERLAB_KUBERNETES) destroy
	$(KIND) delete cluster --name $(CLUSTER_NAME)

.PHONY: install-sart
install-sart: $(MANIFESTDIR)/crd/sart.yaml
ifeq ($(MODE),lb)
	$(KUSTOMIZE) build $(LB_MANIFESTDIR) | $(KUBECTL) apply -f -
else
	$(KUSTOMIZE) build $(CNI_MANIFESTDIR) | $(KUBECTL) apply -f -
endif

.PHONY: sample
sample:
ifeq ($(MODE), lb)
	$(KUSTOMIZE) build $(MANIFESTDIR)/sample | $(KUBECTL) apply -f -
else
	$(KUSTOMIZE) build $(CNI_MANIFESTDIR)/sample | $(KUBECTL) apply -f -
endif


$(MANIFESTDIR)/crd/sart.yaml:
	@echo "PLEASE GENERATE CRD MANIFESTS FIRST"
	exit 1

$(MANIFESTDIR)/certs/tls.cert:
	@echo "PLEASE GENERATE CERTIFICATES FIRST"
	exit 1

.PHONY: setup
setup: $(KIND) $(KUBECTL) $(KUSTOMIZE) $(CILIUM_CLI) $(CONTAINERLAB) $(HELM)

$(KIND):
	mkdir -p $(dir $@)
	curl -sfL -o $@ https://github.com/kubernetes-sigs/kind/releases/download/v$(KIND_VERSION)/kind-linux-amd64
	chmod a+x $@

$(KUBECTL):
	mkdir -p $(dir $@)
	curl -sfL -o $@ https://dl.k8s.io/release/v$(KUBERNETES_VERSION)/bin/linux/amd64/kubectl
	chmod a+x $@

$(KUSTOMIZE):
	mkdir -p $(dir $@)
	curl -sfL https://github.com/kubernetes-sigs/kustomize/releases/download/kustomize%2Fv$(KUSTOMIZE_VERSION)/kustomize_v$(KUSTOMIZE_VERSION)_linux_amd64.tar.gz | tar -xz -C $(BINDIR)
	chmod a+x $@

$(CONTAINERLAB):
	mkdir -p $(dir $@)
	curl -sfL https://github.com/srl-labs/containerlab/releases/download/v$(CONTAINERLAB_VERSION)/containerlab_$(CONTAINERLAB_VERSION)_Linux_amd64.tar.gz | tar -xz -C $(BINDIR)
	rm -rf $(BINDIR)/lab-examples
	rm -rf $(BINDIR)/templates
	chmod a+x $@

$(CILIUM_CLI):
	mkdir -p $(dir $@)
	curl -sfL https://github.com/cilium/cilium-cli/releases/download/v$(CILIUM_CLI_VERSION)/cilium-linux-amd64.tar.gz | tar -xz -C $(BINDIR)
	chmod a+x $@

$(HELM):
	mkdir -p $(dir $@)
	curl -sfL https://get.helm.sh/helm-v$(HELM_VERSION)-linux-amd64.tar.gz | tar -zx -C $(BINDIR)
	cp $(BINDIR)/linux-amd64/helm $(BINDIR)
	chmod a+x $@
	rm -rf $(BINDIR)/linux-amd64
