# Build the manager binary
ARG RUST_VERSION=1.72.0

# BUILDPLATFORM = linux/amd64

FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION} as builder

COPY ../controller /work/controller
COPY ../proto /work/proto

RUN apt update -y && \
	apt install -y protobuf-compiler libprotobuf-dev

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

WORKDIR /work/controller
RUN cargo build && \
		cp /work/controller/target/debug/controller /usr/local/bin/controller

FROM debian:stable

RUN apt update -y && \
	apt install -y iproute2

COPY --from=builder /usr/local/bin/controller /usr/local/bin/controller

ENTRYPOINT [ "/usr/local/bin/controller" ]
