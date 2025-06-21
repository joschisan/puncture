FROM rustlang/rust:nightly-slim as builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY puncture-api-core/ ./puncture-api-core/
COPY puncture-cli/ ./puncture-cli/
COPY puncture-cli-core/ ./puncture-cli-core/
COPY puncture-client/ ./puncture-client/
COPY puncture-core/ ./puncture-core/
COPY puncture-daemon/ ./puncture-daemon/
COPY puncture-testing/ ./puncture-testing/

RUN cargo build --release --bin puncture-daemon

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -u 1000 puncture \
    && mkdir -p /data/puncture /data/ldk \
    && chown -R puncture:puncture /data

COPY --from=builder /app/target/release/puncture-daemon /usr/local/bin/puncture-daemon

USER puncture

EXPOSE 8080 9735

ENTRYPOINT ["puncture-daemon"]