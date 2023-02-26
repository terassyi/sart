RUSTUP := rustup
CARGO := cargo
GOBGP_VERSION := 3.10.0
GRPCURL_VERSION := 1.8.7

.PHONY: setup
setup: setup-rust-tools setup-protoc

.PHONY: setup-rust-tools
setup-rust-tools:
	$(RUSTUP) component add llvm-tools-preview
	$(RUSTUP) component add rustfmt
	$(CARGO) install rustfilt

.PHONY: setup-protoc
setup-protoc:
	sudo apt install -y protobuf-compiler libprotobuf-dev

.PHONY: setup-dev
setup-dev:
	sudo apt install -y frr jq iproute2
	sudo wget -P /tmp https://github.com/osrg/gobgp/releases/download/v${GOBGP_VERSION}/gobgp_${GOBGP_VERSION}_linux_amd64.tar.gz
	sudo tar -zxvf /tmp/gobgp_${GOBGP_VERSION}_linux_amd64.tar.gz -C /usr/bin/
	sudo wget -P /tmp https://github.com/fullstorydev/grpcurl/releases/download/v${GRPCURL_VERSION}/grpcurl_${GRPCURL_VERSION}_linux_x86_64.tar.gz
	sudo tar -zxvf /tmp/grpcurl_${GRPCURL_VERSION}_linux_x86_64.tar.gz -C /usr/bin/

.PHONY: build
build: build-daemon build-cli

.PHONY: build-daemon
build-daemon:
	cd sartd; $(CARGO) build --verbose

.PHONY: build-cli
build-cli:
	cd sart; $(CARGO) build --verbose


.PHONY: fmt
fmt:
	cd sartd; $(CARGO) fmt

.PHONY: test
test: unit-test

.PHONY: unit-test
unit-test:
	cd sartd; $(CARGO) test

.PHONY: integration-test
integration-test:
	sartd/test/run-integration.sh

.PHONY: dev-container
dev-container:
	docker run -it --privileged --rm --name sart-dev -p 8080:8080 -w /work/sart -v `pwd`:/work/sart ghcr.io/terassyi/terakoya:0.1.2 bash

.PHONY: in-container
in-container:
	docker exec -it sart-dev bash
