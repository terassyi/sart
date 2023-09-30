CLUSTER_NAME := sart

KIND_VERSION := 0.20.0
KUBERNETES_VERSION := 1.28.0
CILIUM_VERSION := 1.14.1
CILIUM_CLI_VERSION := 0.15.7
KUSTOMIZE_VERSION := 5.1.1
CONTAINERLAB_VERSION := 0.44.3
HELM_VERSION := 3.12.3

SUDO ?= sudo

BINDIR := $(abspath $(PWD)/bin)
MANIFESTDIR := $(abspath $(PWD)/manifests)

KIND := $(BINDIR)/kind
KUBECTL := $(BINDIR)/kubectl
KUSTOMIZE := $(BINDIR)/kustomize
CONTAINERLAB := $(BINDIR)/containerlab

export KUBECTL

KIND_CONFIG = kind-config.yaml
CONTAINERLAB_CONFIG = lab.yaml

.PHONY: setup
setup: $(KIND) $(KUBECTL) $(KUSTOMIZE) $(CILIUM_CLI) $(CONTAINERLAB) $(HELM)

.PHONY: cluster
cluster: setup
	$(SUDO) sysctl -w fs.inotify.max_user_instances=512
	$(SUDO) sysctl -w fs.inotify.max_user_watches=65536

	@echo "CREATE THE KIND CLUSTER"
	$(KIND) create cluster --image kindest/node:v$(KUBERNETES_VERSION) --config=$(KIND_CONFIG) --name $(CLUSTER_NAME)

	@echo "CREATE TOPOLOGY"
	$(SUDO) $(CONTAINERLAB) -t $(CONTAINERLAB_CONFIG) deploy

	@echo "ADD LABELS"
	$(KUBECTL) label nodes --overwrite sart-worker bgp=a
	$(KUBECTL) label nodes --overwrite sart-worker2 bgp=a
	$(KUBECTL) label nodes --overwrite sart-worker3 bgp=a

.PHONY: down
down:
	$(SUDO) $(CONTAINERLAB) -t $(CONTAINERLAB_CONFIG) destroy
	$(KIND) delete cluster --name $(CLUSTER_NAME)


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
