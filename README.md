# FindVerse

[English](README.md) | [简体中文](README.zh-CN.md)

FindVerse is a self-hosted search stack with a Dockerized control plane and independent crawler workers. The control plane stays easy to bring up on a single machine, while crawler nodes can scale and upgrade separately.

## Quick Start

1. Copy the environment template.

```bash
cp .env.example .env
```

2. Set at least these values in `.env`.

- `FINDVERSE_FRONTEND_ORIGIN`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`
- `FINDVERSE_POSTGRES_PASSWORD`

3. Start the control plane.

```bash
docker compose up -d --build
```

4. Set one shared crawler auth key in `/console -> Settings`, then install a crawler node.

```bash
tmp="$(mktemp)" && \
{ curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh -o "$tmp" || \
  curl -fsSL https://gh-proxy.net/https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh -o "$tmp"; } && \
sudo bash "$tmp" -- --server https://search.example.com/api --crawler-key "<crawler-key>" --max-jobs 16 --skip-browser-install; \
status=$?; rm -f "$tmp"; [ $status -eq 0 ]
```

Control-plane data is stored under `./data`. Docker Compose deploys only the control plane; crawler nodes are separate host services. If a crawler node still predates the split crawler package, run `install-crawler.sh` once manually on that machine before using console-triggered remote updates. On low-memory hosts, prefer `COMPOSE_PARALLEL_LIMIT=1 docker compose up -d --build`. Rust service images also default to `FINDVERSE_CARGO_BUILD_JOBS=1`.

## Documentation

- Deployment and operations: [docs/deployment.md](docs/deployment.md)
- Architecture overview: [docs/architecture.md](docs/architecture.md)
