RUSTUP := rustup
CARGO := cargo

.PHONY: setup
setup: setup-rust-tools

.PHONY: setup-rust-tools
setup-rust-tools:
	$(RUSTUP) component add llvm-tools-preview
	$(RUSTUP) component add rustfmt
	$(CARGO) install rustfilt

.PHONY: fmt
fmt:
	cd sartd; $(CARGO) fmt

.PHONY: test
test:
	cd sartd; $(CARGO) test
