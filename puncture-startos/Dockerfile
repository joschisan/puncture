FROM rust:bookworm AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY puncture-cli/ ./puncture-cli/
COPY puncture-cli-core/ ./puncture-cli-core/
COPY puncture-client/ ./puncture-client/
COPY puncture-client-core/ ./puncture-client-core/
COPY puncture-client-db/ ./puncture-client-db/
COPY puncture-core/ ./puncture-core/
COPY puncture-daemon/ ./puncture-daemon/
COPY puncture-daemon-db/ ./puncture-daemon-db/
COPY puncture-payment-request/ ./puncture-payment-request/
COPY puncture-testing/ ./puncture-testing/

RUN cargo build --release --bin puncture-daemon --bin puncture-cli

FROM debian:bookworm-slim

LABEL start9.service-name="puncture"
LABEL start9.service-version="0.1.0"

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -u 1000 puncture \
    && mkdir -p /data/puncture /data/ldk \
    && chown -R puncture:puncture /data

COPY --from=builder /app/target/release/puncture-daemon /usr/local/bin/puncture-daemon
COPY --from=builder /app/target/release/puncture-cli /usr/local/bin/puncture-cli

COPY puncture-startos/docker_entrypoint.sh /usr/local/bin/docker_entrypoint.sh

RUN chmod +x /usr/local/bin/docker_entrypoint.sh

USER puncture

ENTRYPOINT ["/usr/local/bin/docker_entrypoint.sh"] 