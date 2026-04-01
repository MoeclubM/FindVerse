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
COPY services/control-api services/control-api
COPY services/query-api/src services/query-api/src
COPY services/task-api/src services/task-api/src
COPY services/scheduler/src services/scheduler/src
COPY services/crawler/src services/crawler/src

RUN cargo build --release -p findverse-control-api

FROM quay.io/fedora/fedora:43 AS runner
WORKDIR /app
USER 0

COPY --from=builder /app/target/release/findverse-control-api /usr/local/bin/findverse-control-api
COPY services/api/migrations /app/migrations
COPY services/api/fixtures/bootstrap_documents.json /opt/findverse/bootstrap_documents.json
COPY services/api/fixtures/developer_store.json /opt/findverse/developer_store.json
COPY services/api/fixtures/dev_auth_store.json /opt/findverse/dev_auth_store.json

ENV FINDVERSE_CONTROL_API_BIND=0.0.0.0:8080
ENV FINDVERSE_INDEX_PATH=/opt/findverse/bootstrap_documents.json
ENV FINDVERSE_DEVELOPER_STORE=/opt/findverse/developer_store.json
ENV FINDVERSE_DEV_AUTH_STORE=/opt/findverse/dev_auth_store.json
ENV FINDVERSE_POSTGRES_URL=postgres://findverse:findverse@postgres:5432/findverse
ENV FINDVERSE_REDIS_URL=redis://valkey:6379/0
ENV FINDVERSE_OPENSEARCH_URL=http://opensearch:9200

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/findverse-control-api"]
