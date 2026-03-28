FROM quay.io/fedora/fedora:43 AS builder
WORKDIR /app

RUN dnf install -y cargo rustc rustfmt gcc gcc-c++ make pkgconf-pkg-config perl \
    && dnf clean all

COPY Cargo.toml Cargo.lock ./
COPY crates/findverse-common/Cargo.toml crates/findverse-common/Cargo.toml
COPY services/api/Cargo.toml services/api/Cargo.toml
COPY services/control-api/Cargo.toml services/control-api/Cargo.toml
COPY services/query-api/Cargo.toml services/query-api/Cargo.toml
COPY services/crawler/Cargo.toml services/crawler/Cargo.toml
COPY crates/findverse-common/src crates/findverse-common/src
COPY services/api services/api
COPY services/control-api/src services/control-api/src
COPY services/query-api/src services/query-api/src
COPY services/crawler services/crawler
COPY config config

RUN cargo build --release -p findverse-crawler

FROM quay.io/fedora/fedora-minimal:43 AS runner
WORKDIR /app
USER 0
COPY --from=builder /app/target/release/findverse-crawler /usr/local/bin/findverse-crawler
COPY config config

ENTRYPOINT ["/usr/local/bin/findverse-crawler"]
CMD ["--help"]
