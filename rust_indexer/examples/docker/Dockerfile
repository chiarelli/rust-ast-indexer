FROM rust:1.94-bookworm as builder

WORKDIR /build

RUN apt-get update && apt-get install -y \
    git \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && mkdir tests
COPY src ./src
COPY tests ./tests

RUN cargo build --release --features parsing

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    nodejs \
    coreutils \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/rust_indexer /usr/local/bin/

RUN chmod +x /usr/local/bin/rust_indexer

RUN useradd -m -s /bin/bash appuser && chown -R appuser:appuser /app
USER appuser