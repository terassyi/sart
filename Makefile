RUSTUP := rustup
CARGO := cargo
IMAGE_VERSION := dev
PROJECT := github.com/terassyi/sart

RELEASE_VERSION=
.PHONY: release-pr
release-pr: validate-release-version cargo-bump

	git checkout main
	git pull origin main
	git checkout -b bump-v$(RELEASE_VERSION)

	yq -i '.images[].newTag="$(RELEASE_VERSION)"' controller/config/release/kustomization.yaml
	cd sartd; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update
	cd sart; $(CARGO) bump $(RELEASE_VERSION) && $(CARGO) update

	$(MAKE) build-image
	cd controller; make docker-build

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


MANIFESTS_DIR := $(PWD)/manifests
CRD_DIR := $(MANIFESTS_DIR)/crd
CERT_DIR := $(MANIFESTS_DIR)/certs

.PHONY: crd
crd: $(CRD_DIR)/sart.yaml

.PHONY: clean-crd
clean-crd:
	rm $(CRD_DIR)/sart.yaml || true

$(CRD_DIR)/sart.yaml:
	mkdir -p $(CRD_DIR)
	cd sartd; $(CARGO) run --bin crdgen > $(CRD_DIR)/sart.yaml

.PHONY: certs
certs: $(CERT_DIR)/tls.cert $(CERT_DIR)/tls.key manifests/webhook/admission_webhook_patch.yaml

.PHONY: clean-certs
clean-certs:
	rm -f $(CERT_DIR)/tls.cert  $(CERT_DIR)/tls.key || true
	rm -f manifests/webhook/admission_webhook_patch.yaml || true

$(CERT_DIR)/tls.cert:
	mkdir -p $(CERT_DIR)
	cd sartd; $(CARGO) run --bin certgen -- --out-dir $(CERT_DIR)

manifests/webhook/admission_webhook_patch.yaml: $(CERT_DIR)/tls.cert manifests/webhook/admission_webhook_patch.yaml.tmpl
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
integration-test:
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
