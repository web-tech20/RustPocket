FROM rust:1.100 AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch --locked
COPY src ./src
RUN cargo build --locked --release

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates docker.io \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rustpocket-server /usr/local/bin/rustpocket-server

ENV RUSTPOCKET_BIND=0.0.0.0:8080
ENV RUSTPOCKET_DATABASE=/data/rustpocket.db
ENV RUSTPOCKET_SANDBOX=docker
ENV RUSTPOCKET_SANDBOX_IMAGE=rustpocket-sandbox:latest

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/rustpocket-server"]
