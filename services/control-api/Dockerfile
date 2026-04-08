FROM rust:1-slim-bookworm AS builder
WORKDIR /app
ARG FINDVERSE_CARGO_BUILD_JOBS=1
ENV CARGO_BUILD_JOBS=${FINDVERSE_CARGO_BUILD_JOBS}

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
COPY services/blob-storage/src/main.rs services/blob-storage/src/main.rs
COPY services/control-api/src services/control-api/src
COPY services/projector/src/main.rs services/projector/src/main.rs
COPY services/query-api/src/main.rs services/query-api/src/main.rs
COPY services/task-api/src/main.rs services/task-api/src/main.rs
COPY services/scheduler/src/main.rs services/scheduler/src/main.rs

RUN cargo build --release -p findverse-control-api

FROM debian:bookworm-slim AS runner
WORKDIR /app
USER 0

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/findverse-control-api /usr/local/bin/findverse-control-api
COPY services/api/migrations /app/migrations
COPY services/api/fixtures/bootstrap_documents.json /opt/findverse/bootstrap_documents.json

ENV FINDVERSE_CONTROL_API_BIND=0.0.0.0:8080
ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_OPENSEARCH_URL=http://opensearch:9200
ENV FINDVERSE_BLOB_STORAGE_URL=http://blob-storage:8090

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/findverse-control-api"]
