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
COPY services/bootstrap/src services/bootstrap/src
COPY services/blob-storage/src services/blob-storage/src
COPY services/control-api/src services/control-api/src
COPY services/projector/src services/projector/src
COPY services/query-api/src services/query-api/src
COPY services/task-api/src services/task-api/src
COPY services/scheduler/src services/scheduler/src

RUN cargo build --release \
    -p findverse-bootstrap \
    -p findverse-blob-storage \
    -p findverse-control-api \
    -p findverse-projector \
    -p findverse-query-api \
    -p findverse-task-api \
    -p findverse-scheduler

FROM debian:bookworm-slim AS runner-base
WORKDIR /app
USER 0

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libgcc-s1 \
    && rm -rf /var/lib/apt/lists/*

FROM runner-base AS runner-with-wget

RUN apt-get update \
    && apt-get install -y --no-install-recommends wget \
    && rm -rf /var/lib/apt/lists/*

FROM runner-with-wget AS blob-storage-runner
COPY --from=builder /app/target/release/findverse-blob-storage /usr/local/bin/findverse-blob-storage

ENV FINDVERSE_BLOB_STORAGE_BIND=0.0.0.0:8090
ENV FINDVERSE_BLOB_STORE_DIR=/var/lib/findverse/blobs

EXPOSE 8090
ENTRYPOINT ["/usr/local/bin/findverse-blob-storage"]

FROM runner-base AS bootstrap-runner
COPY --from=builder /app/target/release/findverse-bootstrap /usr/local/bin/findverse-bootstrap
COPY services/api/fixtures/bootstrap_documents.json /opt/findverse/bootstrap_documents.json

ENV FINDVERSE_INDEX_PATH=/opt/findverse/bootstrap_documents.json
ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_OPENSEARCH_URL=http://opensearch:9200
ENV FINDVERSE_BLOB_STORAGE_URL=http://blob-storage:8090

ENTRYPOINT ["/usr/local/bin/findverse-bootstrap"]

FROM runner-base AS control-api-runner
COPY --from=builder /app/target/release/findverse-control-api /usr/local/bin/findverse-control-api

ENV FINDVERSE_CONTROL_API_BIND=0.0.0.0:8080
ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_OPENSEARCH_URL=http://opensearch:9200
ENV FINDVERSE_BLOB_STORAGE_URL=http://blob-storage:8090

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/findverse-control-api"]

FROM runner-base AS query-api-runner
COPY --from=builder /app/target/release/findverse-query-api /usr/local/bin/findverse-query-api

ENV FINDVERSE_QUERY_API_BIND=0.0.0.0:8081
ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_OPENSEARCH_URL=http://opensearch:9200
ENV FINDVERSE_BLOB_STORAGE_URL=http://blob-storage:8090

EXPOSE 8081
ENTRYPOINT ["/usr/local/bin/findverse-query-api"]

FROM runner-base AS task-api-runner
COPY --from=builder /app/target/release/findverse-task-api /usr/local/bin/findverse-task-api

ENV FINDVERSE_TASK_API_BIND=0.0.0.0:8082
ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_BLOB_STORAGE_URL=http://blob-storage:8090

EXPOSE 8082
ENTRYPOINT ["/usr/local/bin/findverse-task-api"]

FROM runner-base AS scheduler-runner
COPY --from=builder /app/target/release/findverse-scheduler /usr/local/bin/findverse-scheduler

ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_OPENSEARCH_URL=http://opensearch:9200
ENV FINDVERSE_BLOB_STORAGE_URL=http://blob-storage:8090

ENTRYPOINT ["/usr/local/bin/findverse-scheduler"]

FROM runner-base AS projector-runner
COPY --from=builder /app/target/release/findverse-projector /usr/local/bin/findverse-projector

ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_OPENSEARCH_URL=http://opensearch:9200
ENV FINDVERSE_BLOB_STORAGE_URL=http://blob-storage:8090

ENTRYPOINT ["/usr/local/bin/findverse-projector"]
