# Stage 1: Build frontend
FROM node:24-alpine AS frontend

RUN npm install -g bun@latest

WORKDIR /build/frontend-panel
COPY frontend-panel/package.json frontend-panel/bun.lock* ./
RUN bun install --frozen-lockfile

COPY frontend-panel/ ./
RUN bun run build

# Stage 2: Build Rust binary
FROM rust:1-alpine AS builder

RUN apk add --no-cache build-base pkgconfig sqlite-dev curl

WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY migration/ migration/
COPY api-docs-macros/ api-docs-macros/

# Pre-build dependencies (cache layer)
RUN mkdir src && echo 'fn main() {}' > src/main.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src

COPY src/ src/
COPY build.rs ./
COPY --from=frontend /build/frontend-panel/dist/ frontend-panel/dist/

ARG CARGO_FEATURES="server,cli"
ENV RUSTFLAGS="-C link-arg=-s"

RUN cargo build --release --features "${CARGO_FEATURES}"

# Stage 3: Alpine runtime
FROM alpine:3.22

RUN apk add --no-cache ca-certificates sqlite-libs

LABEL maintainer="AptS:1547 <apts-1547@esaps.net>"
LABEL org.opencontainers.image.title="AsterDrive"
LABEL org.opencontainers.image.description="Self-hosted cloud storage system built with Rust"
LABEL org.opencontainers.image.source="https://github.com/AptS-1547/AsterDrive"
LABEL org.opencontainers.image.license="MIT"

COPY --from=builder /build/target/release/aster_drive /aster_drive

VOLUME ["/data"]
EXPOSE 3000

ENV ASTER__SERVER__HOST=0.0.0.0

ENTRYPOINT ["/aster_drive"]
