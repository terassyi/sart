ARG RUST_VERSION=1.76.0

# BUILDPLATFORM = linux/amd64

FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION} as builder

WORKDIR /home
COPY ./sartd /home/sartd
COPY ./sart /home/sart
COPY ./sartcni /home/sartcni
COPY ./proto /home/proto

RUN apt update -y && \
	apt install -y protobuf-compiler libprotobuf-dev clang llvm mold gcc-multilib

ENV CC_aarch64_unknown_linux_musl=clang
ENV AR_aarch64_unknown_linux_musl=llvm-ar
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-Clink-self-contained=yes -Clinker=rust-lld"

ENV CC_x86_64_unknown_linux_musl=clang
ENV AR_x86_64_unknown_linux_musl=llvm-ar
ENV CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS="-Clink-self-contained=yes -Clinker=rust-lld"

ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
	"linux/arm64") echo aarch64-unknown-linux-musl > /rust_target.txt ;; \
	"linux/amd64") echo x86_64-unknown-linux-musl > /rust_target.txt ;; \
	*) exit 1 ;; \
	esac

RUN rustup target add $(cat /rust_target.txt)

RUN cd sartd; cargo build --release --target $(cat /rust_target.txt) && \
	cp /home/sartd/target/$(cat /rust_target.txt)/release/sartd /usr/local/bin/sartd && \
	cargo build --release --bin cni-installer --target $(cat /rust_target.txt) && \
	cp /home/sartd/target/$(cat /rust_target.txt)/release/cni-installer /usr/local/bin/cni-installer
RUN cd sart; cargo build --release --target $(cat /rust_target.txt) && \
	cp /home/sart/target/$(cat /rust_target.txt)/release/sart /usr/local/bin/sart
RUN cd sartcni; cargo build --release --target $(cat /rust_target.txt) && \
	cp /home/sartcni/target/$(cat /rust_target.txt)/release/sart-cni /usr/local/bin/sart-cni

FROM debian:stable

RUN apt update -y && \
	apt install -y iproute2

COPY --from=builder /usr/local/bin/sartd /usr/local/bin/sartd
COPY --from=builder /usr/local/bin/sart /usr/local/bin/sart
COPY --from=builder /usr/local/bin/cni-installer /usr/local/bin/cni-installer

COPY --from=builder /usr/local/bin/sart-cni /host/opt/cni/bin/sart-cni
COPY netconf.json /host/etc/cni/net.d/10-sart.conflist
