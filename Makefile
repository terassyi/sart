RUSTUP := rustup
CARGO := cargo
GOBGP_VERSION := 3.10.0

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
	sudo wget -P /tmp https://github.com/osrg/gobgp/releases/download/v3.10.0/gobgp_${GOBGP_VERSION}_linux_amd64.tar.gz
	tar -zxvf /tmp/gobgp_${GOBGP_VERSION}_linux_amd64.tar.gz -C /usr/bin/

.PHONY: build
build:
	cd sartd; $(CARGO) build --verbose

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



SPEC = "sartd/testdata/tinet/basic/spec.yaml"
.PHONY: tinet
tinet.%:
	tinet ${@:tinet.%=%} -c $(SPEC) | sudo sh -x
