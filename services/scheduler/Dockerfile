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
COPY services/control-api/src services/control-api/src
COPY services/query-api/src services/query-api/src
COPY services/task-api/src services/task-api/src
COPY services/scheduler services/scheduler
COPY services/crawler/src services/crawler/src

RUN cargo build --release -p findverse-scheduler

FROM quay.io/fedora/fedora:43 AS runner
WORKDIR /app
USER 0

COPY --from=builder /app/target/release/findverse-scheduler /usr/local/bin/findverse-scheduler
COPY services/api/migrations /app/migrations

ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_OPENSEARCH_URL=http://opensearch:9200
ENV FINDVERSE_BLOB_STORE_DIR=/var/lib/findverse/blobs

ENTRYPOINT ["/usr/local/bin/findverse-scheduler"]
