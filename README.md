# FindVerse

[English](README.md) | [简体中文](README.zh-CN.md)

FindVerse is a self-hosted search stack with a split control plane, query API, web UI, and independent crawler workers. It is designed to stay easy to deploy on a single machine while keeping the crawler and indexing pipeline extensible.

## Highlights

- Public search, suggest, and developer search APIs
- Admin console for crawler workers, crawl rules, jobs, and indexed documents
- Independent crawler workers that can be scaled and upgraded separately
- Recursive crawl controls with domain and expansion limits
- Optional OpenAI-compatible LLM filtering before indexing
- Simple Docker Compose deployment for the main stack

## Repository Layout

- `apps/web`: React SPA for `/`, `/dev`, and `/console`
- `services/control-api`: admin, developer, crawler control, frontier, jobs, rules, and documents
- `services/query-api`: public search, suggest, and developer search
- `services/crawler`: crawler worker and local crawl tooling
- `services/api`: shared backend library used by the split APIs

## Quick Start

1. Copy the shared environment template.

```bash
cp .env.example .env
```

2. Update at least these values in `.env`.

- `FINDVERSE_FRONTEND_ORIGIN`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`
- `FINDVERSE_POSTGRES_PASSWORD`

3. Build and start the main stack.

```bash
docker compose up -d --build
```

Main service data is persisted under `./data` and will be created automatically by Docker when the stack starts.

## Crawler Worker

Set one shared crawler auth key in `/console -> Settings`, then install or update a crawler worker directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --channel release --concurrency 16 --skip-browser-install
```

The first install auto-generates `crawler_id` locally and writes it into `/etc/findverse-crawler/crawler.env`. Re-running the same command updates the node in place and reuses the saved id.

Only use the development channel when you explicitly want the latest successful CI build:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo env GITHUB_TOKEN=<TOKEN> bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --channel dev --concurrency 16 --skip-browser-install
```

`--channel release` does not need a token. `--channel dev` does, because GitHub Actions artifact downloads require authenticated API access.

## Development Notes

- Main stack deployment is `docker compose up -d --build`
- Crawler nodes are intended to run as host services, not inside the main production compose stack
- Current CI only runs regular validation from `.github/workflows/_validate.yml`
- A separate workflow builds the crawler development artifact used by `install-crawler.sh --channel dev`

## Documentation

- Deployment and operations: [docs/deployment.md](docs/deployment.md)
- Architecture overview: [docs/architecture.md](docs/architecture.md)
