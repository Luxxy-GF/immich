FROM rust:1.88-bookworm AS builder

WORKDIR /usr/src/app

RUN apt-get update && \
    apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY server-rs/Cargo.toml server-rs/Cargo.lock ./server-rs/
WORKDIR /usr/src/app/server-rs

RUN mkdir -p src && \
    printf 'fn main() {}\n' > src/main.rs && \
    cargo build --release && \
    rm -rf src

COPY server-rs/src /usr/src/app/server-rs/src
COPY server-rs/migrations /usr/src/app/server-rs/migrations
COPY server/package.json /usr/src/app/server/package.json
COPY server-rs/.env /usr/src/app/server-rs/.env

RUN cargo clean && cargo build --release --locked

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates ffmpeg tini && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

COPY --from=builder /usr/src/app/server-rs/target/release/server-rs /usr/local/bin/server-rs

ENV PORT=2283 \
    IMMICH_MEDIA_LOCATION=/data

VOLUME /data
EXPOSE 2283

ENTRYPOINT ["tini", "--", "/usr/local/bin/server-rs"]
