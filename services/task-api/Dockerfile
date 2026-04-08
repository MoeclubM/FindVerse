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
COPY services/api services/api
COPY services/bootstrap/src services/bootstrap/src
COPY services/blob-storage/src services/blob-storage/src
COPY services/control-api/src services/control-api/src
COPY services/projector/src services/projector/src
COPY services/query-api/src services/query-api/src
COPY services/task-api/src services/task-api/src
COPY services/scheduler/src services/scheduler/src

RUN cargo build --release -p findverse-task-api

FROM debian:bookworm-slim AS runner
WORKDIR /app
USER 0

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/findverse-task-api /usr/local/bin/findverse-task-api
COPY services/api/migrations /app/migrations

ENV FINDVERSE_TASK_API_BIND=0.0.0.0:8082
ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_BLOB_STORAGE_URL=http://blob-storage:8090

EXPOSE 8082
ENTRYPOINT ["/usr/local/bin/findverse-task-api"]
