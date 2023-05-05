ARG RUST_VERSION=1.68.0

FROM rust:${RUST_VERSION} as builder

WORKDIR /home
COPY . /home/

RUN apt update -y && \
	apt install -y protobuf-compiler libprotobuf-dev
RUN make release-build

FROM debian:stable

COPY --from=builder /home/sartd/target/release/sartd /usr/local/bin/sartd
COPY --from=builder /home/sart/target/release/sart /usr/local/bin/sart
