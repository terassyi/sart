RUSTUP := rustup
CARGO := cargo

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
	sudo apt install -y frr gobgpd jq iproute2

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
