RUSTUP := rustup
CARGO := cargo
GOBGP_VERSION := 3.10.0
GRPCURL_VERSION := 1.8.7
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
setup: setup-rust-tools setup-grpc

.PHONY: setup-rust-tools
setup-rust-tools:
	$(RUSTUP) component add llvm-tools-preview
	$(RUSTUP) component add rustfmt
	$(CARGO) install rustfilt

.PHONY: setup-grpc
setup-grpc:
	sudo apt install -y protobuf-compiler libprotobuf-dev
	go install github.com/bufbuild/buf/cmd/buf@latest
	go install google.golang.org/protobuf/cmd/protoc-gen-go@latest
	go install github.com/bufbuild/connect-go/cmd/protoc-gen-connect-go@latest
	go install google.golang.org/grpc/cmd/protoc-gen-go-grpc@latest

.PHONY: setup-dev
setup-dev:
	sudo apt install -y frr jq iproute2
	sudo wget -P /tmp https://github.com/osrg/gobgp/releases/download/v${GOBGP_VERSION}/gobgp_${GOBGP_VERSION}_linux_amd64.tar.gz
	sudo tar -zxvf /tmp/gobgp_${GOBGP_VERSION}_linux_amd64.tar.gz -C /usr/bin/
	sudo wget -P /tmp https://github.com/fullstorydev/grpcurl/releases/download/v${GRPCURL_VERSION}/grpcurl_${GRPCURL_VERSION}_linux_x86_64.tar.gz
	sudo tar -zxvf /tmp/grpcurl_${GRPCURL_VERSION}_linux_x86_64.tar.gz -C /usr/bin/

.PHONY: release-build
release-build:
	cd sartd; $(CARGO) build --release
	cd ..
	cd sart; $(CARGO) build --release

.PHONY: build
build: build-daemon build-cli

.PHONY: build-daemon
build-daemon: crd certs
	cd sartd; $(CARGO) build --verbose

.PHONY: build-cli
build-cli:
	cd sart; $(CARGO) build --verbose

.PHONY: build-proto
build-proto:
	protoc -Iproto --go_out=./controller/pkg/proto bgp.proto
	protoc -Iproto --go-grpc_out=./controller/pkg/proto bgp.proto
	protoc -Iproto --go_out=./controller/pkg/proto fib.proto
	protoc -Iproto --go-grpc_out=./controller/pkg/proto fib.proto

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
certs: $(CERT_DIR)/tls.cert $(CERT_DIR)/tls.key manifests/webhook/validating_webhook_patch.yaml

.PHONY: clean-certs
clean-certs:
	rm -f $(CERT_DIR)/tls.cert  $(CERT_DIR)/tls.key || true
	rm -f manifests/webhook/validating_webhook_patch.yaml || true

$(CERT_DIR)/tls.cert:
	mkdir -p $(CERT_DIR)
	cd sartd; $(CARGO) run --bin certgen -- --out-dir $(CERT_DIR)

manifests/webhook/validating_webhook_patch.yaml: $(CERT_DIR)/tls.key $(CERT_DIR)/tls.cert manifests/webhook/validating_webhook_patch.yaml.tmpl
	sed "s/%CACERT%/$$(base64 -w0 < $<)/g" $@.tmpl > $@

.PHONY: fmt
fmt:
	cd sartd; $(CARGO) fmt

.PHONY: test
test: unit-test controller-test e2e-test

.PHONY: unit-test
unit-test:
	cd sartd; $(CARGO) test

.PHONY: e2e-test
e2e-test: sart-e2e-test controller-e2e-test

.PHONY: sart-e2e-test
sart-e2e-test:
	cd e2e; go test -v ./sartd

.PHONY: controller-e2e-test
controller-e2e-test:
	cd e2e/controller; make start
	cd e2e/controller; make install
	cd e2e/controller; make test
	cd e2e/controller; make stop

.PHONY: controller-test
controller-test:
	cd controller; make test

.PHONY: dev-container
dev-container:
	docker run -it --privileged --rm --name sart-dev -p 8080:8080 -w /work/sart -v `pwd`:/work/sart ghcr.io/terassyi/terakoya:0.1.2 bash

.PHONY: in-container
in-container:
	docker exec -it sart-dev bash

.PHONY: build-image
build-image:
	docker build -t sart:${IMAGE_VERSION} .

.PHONY: build-dev-image
build-dev-image:
	docker build -t sart:${IMAGE_VERSION} -f Dockerfile.dev .


CERT_MANAGER_VERSION := 1.11.2
BUILD ?= false
REGISTORY_URL ?= localhost:5005
DEVENV_BGP_ASN ?= 65000
NODE0_ASN ?= 65000
NODE1_ASN ?= 65000
NODE2_ASN ?= 65000
NODE3_ASN ?= 65000
DEVENV_BGP_ADDR ?= ""
NODE0_ADDR ?= ""
NODE1_ADDR ?= ""
NODE2_ADDR ?= ""
NODE3_ADDR ?= ""
CLIENT_ADDR ?= ""
LB_CIDR ?= 10.69.0.0/24
ESCAPED_LB_CIDR ?= "10.69.0.0\/24"

BIN_DIR := bin/
KIND := $(BIN_DIR)/kind
KUBECTL := $(BIN_DIR)/kubectl
KUSTOMZIE := $(BIN_DIR)/kustomize

.PHONY: kind-load
kind-load: build-image
	kind load docker-image sart:dev -n sart

.PHONY: devenv
devenv: crd certs build-dev-image
	$(MAKE) -C devenv cluster
	$(KIND) load docker-image sart:dev -n sart

	$(KUSTOMZIE) build manifests/crd | $(KUBECTL) apply -f -
	$(KUSTOMZIE) build manifests/rbac| $(KUBECTL) apply -f -
	$(KUSTOMZIE) build manifests/webhook | $(KUBECTL) apply -f -
	$(KUSTOMZIE) build manifests/workloads | $(KUBECTL) apply -f -

.PHONY: install-bgp
install-bgp:
	kubectl apply -f manifests/testdata/clusterbgp.yaml

.PHONY: devenv-down
devenv-down: clean-certs
	$(MAKE) -C devenv down

.PHONY: push-image
push-image:
	docker image tag sart:${IMAGE_VERSION} ${REGISTORY_URL}/sart:${IMAGE_VERSION}
	docker image tag sart-controller:${IMAGE_VERSION} ${REGISTORY_URL}/sart-controller:${IMAGE_VERSION}
	docker image tag test-app:${IMAGE_VERSION} ${REGISTORY_URL}/test-app:${IMAGE_VERSION}
	docker push ${REGISTORY_URL}/sart:${IMAGE_VERSION}
	docker push ${REGISTORY_URL}/sart-controller:${IMAGE_VERSION}
	docker push ${REGISTORY_URL}/test-app:${IMAGE_VERSION}

CARGO_BUMP ?= cargo-bump

.PHONY: cargo-bump
cargo-bump:
$(CARGO_BUMP):
ifeq ($(shell which ${CARGO_BUMP}),)
	cargo install cargo-bump
endif
