ARG RUST_VERSION=1.68.0

# BUILDPLATFORM = linux/amd64

FROM --platform=$BUILDPLATFORM rust:${RUST_VERSION} as builder

WORKDIR /home
COPY ./sartd /home/sartd
COPY ./sart /home/sart
COPY ./proto /home/proto

RUN apt update -y && \
	apt install -y protobuf-compiler libprotobuf-dev

ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
	"linux/arm64") echo aarch64-unknown-linux-musl > /rust_target.txt ;; \
	"linux/amd64") echo x86_64-unknown-linux-musl > /rust_target.txt ;; \
	*) exit 1 ;; \
	esac

RUN rustup target add $(cat /rust_target.txt)

RUN cd sartd; cargo build --release --target $(cat /rust_target.txt)
RUN cd sart; cargo build --release --target $(cat /rust_target.txt)

FROM debian:stable

COPY --from=builder /home/sartd/target/$(cat /rust_target.txt)/release/sartd /usr/local/bin/sartd
COPY --from=builder /home/sart/target/$(cat /rust_target.txt)/release/sart /usr/local/bin/sart
