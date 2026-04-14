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
COPY server-rs/scripts /usr/src/app/server-rs/scripts
COPY server/package.json /usr/src/app/server/package.json

RUN cargo clean && cargo build --release --locked

FROM node:24-bookworm AS web-builder

ENV COREPACK_ENABLE_DOWNLOAD_PROMPT=0

RUN corepack enable pnpm

WORKDIR /usr/src/app

COPY package.json pnpm-workspace.yaml pnpm-lock.yaml .pnpmfile.cjs ./
COPY open-api/typescript-sdk ./open-api/typescript-sdk
COPY i18n ./i18n
COPY web ./web

RUN pnpm install --filter @immich/sdk --filter immich-web --frozen-lockfile
RUN pnpm --filter @immich/sdk build
RUN pnpm --filter immich-web build

FROM debian:bookworm-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl ffmpeg tini && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app

COPY --from=builder /usr/src/app/server-rs/target/release/server-rs /usr/local/bin/server-rs
COPY --from=builder /usr/src/app/server-rs/scripts/immich-healthcheck /usr/local/bin/immich-healthcheck
COPY --from=web-builder /usr/src/app/web/build /usr/share/immich-web

RUN chmod +x /usr/local/bin/immich-healthcheck

ENV PORT=2283 \
    IMMICH_MEDIA_LOCATION=/data \
    IMMICH_WEB_ROOT=/usr/share/immich-web

VOLUME /data
EXPOSE 2283

ENTRYPOINT ["tini", "--", "/usr/local/bin/server-rs"]
