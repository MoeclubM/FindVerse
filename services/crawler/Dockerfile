FROM rust:1.93-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY services/api/Cargo.toml services/api/Cargo.toml
COPY services/crawler/Cargo.toml services/crawler/Cargo.toml
COPY services/api services/api
COPY services/crawler services/crawler
COPY config config

RUN cargo build --release -p findverse-crawler

FROM gcr.io/distroless/cc-debian12 AS runner
WORKDIR /app
USER 0
COPY --from=builder /app/target/release/findverse-crawler /usr/local/bin/findverse-crawler
COPY config config

ENTRYPOINT ["/usr/local/bin/findverse-crawler"]
CMD ["--help"]
