FROM rust:1-slim-bookworm AS builder
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config perl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.docker.toml ./Cargo.toml
COPY Cargo.lock ./
COPY crates/findverse-common/Cargo.toml crates/findverse-common/Cargo.toml
COPY services/api/Cargo.toml services/api/Cargo.toml
COPY services/bootstrap/Cargo.toml services/bootstrap/Cargo.toml
COPY services/blob-storage/Cargo.toml services/blob-storage/Cargo.toml
COPY services/control-api/Cargo.toml services/control-api/Cargo.toml
COPY services/projector/Cargo.toml services/projector/Cargo.toml
COPY services/query-api/Cargo.toml services/query-api/Cargo.toml
COPY services/task-api/Cargo.toml services/task-api/Cargo.toml
COPY services/scheduler/Cargo.toml services/scheduler/Cargo.toml
COPY crates/findverse-common/src crates/findverse-common/src
COPY services/api/src services/api/src
COPY services/api/migrations services/api/migrations
COPY services/bootstrap/src/main.rs services/bootstrap/src/main.rs
COPY services/blob-storage/src services/blob-storage/src
COPY services/control-api/src/main.rs services/control-api/src/main.rs
COPY services/projector/src/main.rs services/projector/src/main.rs
COPY services/query-api/src/main.rs services/query-api/src/main.rs
COPY services/task-api/src/main.rs services/task-api/src/main.rs
COPY services/scheduler/src/main.rs services/scheduler/src/main.rs

RUN cargo build --release -p findverse-blob-storage

FROM debian:bookworm-slim AS runner
WORKDIR /app
USER 0

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 wget \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/findverse-blob-storage /usr/local/bin/findverse-blob-storage

ENV FINDVERSE_BLOB_STORAGE_BIND=0.0.0.0:8090
ENV FINDVERSE_BLOB_STORE_DIR=/var/lib/findverse/blobs

EXPOSE 8090
ENTRYPOINT ["/usr/local/bin/findverse-blob-storage"]
