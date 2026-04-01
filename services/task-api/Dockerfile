FROM quay.io/fedora/fedora:43 AS builder
WORKDIR /app

RUN dnf install -y cargo rustc gcc gcc-c++ make pkgconf-pkg-config perl \
    && dnf clean all

COPY Cargo.toml Cargo.lock ./
COPY crates/findverse-common/Cargo.toml crates/findverse-common/Cargo.toml
COPY services/api/Cargo.toml services/api/Cargo.toml
COPY services/control-api/Cargo.toml services/control-api/Cargo.toml
COPY services/query-api/Cargo.toml services/query-api/Cargo.toml
COPY services/task-api/Cargo.toml services/task-api/Cargo.toml
COPY services/scheduler/Cargo.toml services/scheduler/Cargo.toml
COPY services/crawler/Cargo.toml services/crawler/Cargo.toml
COPY crates/findverse-common/src crates/findverse-common/src
COPY services/api services/api
COPY services/task-api services/task-api

RUN cargo build --release -p findverse-task-api

FROM quay.io/fedora/fedora:43 AS runner
WORKDIR /app
USER 0

COPY --from=builder /app/target/release/findverse-task-api /usr/local/bin/findverse-task-api
COPY services/api/migrations /app/migrations

ENV FINDVERSE_TASK_API_BIND=0.0.0.0:8082
ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_BLOB_STORE_DIR=/var/lib/findverse/blobs

EXPOSE 8082
ENTRYPOINT ["/usr/local/bin/findverse-task-api"]
