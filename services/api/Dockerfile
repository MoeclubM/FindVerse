FROM rust:1.93-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY services/api/Cargo.toml services/api/Cargo.toml
COPY services/crawler/Cargo.toml services/crawler/Cargo.toml
COPY services/api services/api
COPY services/crawler services/crawler

RUN cargo build --release -p findverse-api

FROM gcr.io/distroless/cc-debian12 AS runner
WORKDIR /app
USER 0
COPY --from=builder /app/target/release/findverse-api /usr/local/bin/findverse-api
COPY services/api/fixtures/bootstrap_documents.json /opt/findverse/bootstrap_documents.json
COPY services/api/fixtures/developer_store.json /opt/findverse/developer_store.json
COPY services/api/fixtures/crawler_store.json /opt/findverse/crawler_store.json
COPY services/api/fixtures/dev_auth_store.json /opt/findverse/dev_auth_store.json

ENV FINDVERSE_API_BIND=0.0.0.0:8080
ENV FINDVERSE_INDEX_PATH=/data/bootstrap_documents.json
ENV FINDVERSE_DEVELOPER_STORE=/data/developer_store.json
ENV FINDVERSE_CRAWLER_STORE=/data/crawler_store.json
ENV FINDVERSE_DEV_AUTH_STORE=/data/dev_auth_store.json

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/findverse-api"]
