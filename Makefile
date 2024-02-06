RUSTUP := rustup
CARGO := cargo
IMAGE_VERSION := dev
PROJECT := github.com/terassyi/sart

KIND_VERSION := 0.20.0
KUBERNETES_VERSION := 1.28.0
KUSTOMIZE_VERSION := 5.2.1

BINDIR := $(abspath $(PWD)/bin)
KIND := $(BINDIR)/kind
KUBECTL := $(BINDIR)/kubectl
KUSTOMIZE := $(BINDIR)/kustomize

RELEASE_VERSION=
.PHONY: release-pr
release-pr: validate-release-version cargo-bump

	git checkout main
	git pull origin main
	git checkout -b bump-v$(RELEASE_VERSION)

	yq -i '.images[].newTag="$(RELEASE_VERSION)"' ./kustomization.yaml
	cd sartd/src/bgp; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sartd/src/cert; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sartd/src/cmd; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sartd/src/fib; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sartd/src/ipam; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sartd/src/kubernetes; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sartd/src/mock; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sartd/src/proto; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sartd/src/trace; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sartd/src/util; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sartd; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sart; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update

	$(MAKE) build-image

	git add .
	git commit -m "bump v$(RELEASE_VERSION)" --allow-empty

	gh pr create --base main --title "Bump v$(RELEASE_VERSION)" --body ""

.PHONY: release
release: validate-release-version
	git checkout main
	git pull origin main

ifeq ($(shell git log --pretty=format:"%s" -1 --grep="$(RELEASE_VERSION)"),)
	echo "Leatest commit is not release PR"
	exit 1
endif

	git tag -a -m "Release v$(RELEASE_VERSION)" "v$(RELEASE_VERSION)"
	git tag -ln | grep v$(RELEASE_VERSION)
	git push origin "v$(RELEASE_VERSION)"

validate-release-version:
ifndef RELEASE_VERSION
	echo "Please specify a release version"
	exit 1
endif

.PHONY: setup
setup: setup-rust-tools setup-grpc setup-dev

.PHONY: setup-rust-tools
setup-rust-tools:
	$(RUSTUP) component add llvm-tools-preview
	$(RUSTUP) component add rustfmt
	$(CARGO) install rustfilt

.PHONY: setup-grpc
setup-grpc:
	sudo apt install -y protobuf-compiler libprotobuf-dev

.PHONY: setup-dev
setup-dev:
	sudo apt install -y frr jq iproute2

.PHONY: release-build
release-build:
	cd sartd; $(CARGO) build --release
	cd ..
	cd sart; $(CARGO) build --release

.PHONY: build
build: build-daemon build-cli

.PHONY: build-daemon
build-daemon:
	cd sartd; $(CARGO) build --verbose

.PHONY: build-cli
build-cli:
	cd sart; $(CARGO) build --verbose

.PHONY: clean
clean: clean-crd clean-certs
	rm sartd/target/debug/sartd || true
	rm sartd/target/debug/crdgen || true
	rm sartd/target/debug/certgen || true
	rm sart/target/debug/sart || true


MANIFESTS_DIR := $(PWD)/manifests/base
CRD_DIR := $(MANIFESTS_DIR)/crd
CERT_DIR := $(MANIFESTS_DIR)/certs

.PHONY: manifest
manifest: crd certs
	$(KUSTOMIZE) build . > sart.yaml

.PHONY: crd
crd: $(CRD_DIR)/sart.yaml

.PHONY: clean-crd
clean-crd:
	rm $(CRD_DIR)/sart.yaml || true

$(CRD_DIR)/sart.yaml:
	mkdir -p $(CRD_DIR)
	cd sartd; $(CARGO) run --bin crdgen > $(CRD_DIR)/sart.yaml

.PHONY: certs
certs: $(CERT_DIR)/tls.cert $(CERT_DIR)/tls.key manifests/base/webhook/admission_webhook_patch.yaml

.PHONY: clean-certs
clean-certs:
	rm -f $(CERT_DIR)/tls.cert  $(CERT_DIR)/tls.key || true
	rm -f manifests/webhook/admission_webhook_patch.yaml || true

$(CERT_DIR)/tls.cert:
	mkdir -p $(CERT_DIR)
	cd sartd; $(CARGO) run --bin certgen -- --out-dir $(CERT_DIR)

manifests/base/webhook/admission_webhook_patch.yaml: $(CERT_DIR)/tls.cert manifests/base/webhook/admission_webhook_patch.yaml.tmpl
	sed "s/%CACERT%/$$(base64 -w0 < $<)/g" $@.tmpl > $@

.PHONY: fmt
fmt:
	cd sartd; $(CARGO) fmt

.PHONY: test
test: unit-test controller-test e2e-test

.PHONY: unit-test
unit-test:
	cd sartd; $(CARGO) test -p sartd-bgp
	cd sartd; $(CARGO) test -p sartd-cert
	cd sartd; $(CARGO) test -p sartd-fib
	cd sartd; $(CARGO) test -p sartd-ipam
	cd sartd; $(CARGO) test -p sartd-kubernetes
	cd sartd; $(CARGO) test -p sartd-trace
	cd sartd; $(CARGO) test -p sartd-util

.PHONY: integration-test
integration-test: $(KIND) $(KUBECTL) $(KUSTOMIZE)
	cd sartd; $(CARGO) test -p sartd-kubernetes -- --ignored

NO_BUILD:=

.PHONY: build-image
build-image:
	docker build -t sart:${IMAGE_VERSION} .

.PHONY: build-dev-image
build-dev-image:
ifndef NO_BUILD
	docker build -t sart:${IMAGE_VERSION} -f Dockerfile.dev .
endif

CARGO_BUMP ?= cargo-bump

.PHONY: cargo-bump
cargo-bump:
$(CARGO_BUMP):
ifeq ($(shell which ${CARGO_BUMP}),)
	cargo install cargo-bump
endif

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
