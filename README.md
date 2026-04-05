# FindVerse

[English](README.md) | [简体中文](README.zh-CN.md)

FindVerse is a self-hosted search stack with bootstrap, blob-storage, a split control plane, task API, scheduler, projector, query API, web UI, and independent crawler workers. It is designed to stay easy to deploy on a single machine while keeping the crawler and indexing pipeline extensible.

## Highlights

- Public search, suggest, and developer search APIs
- Admin console for crawler workers, crawl rules, jobs, and indexed documents
- Independent crawler workers that can be scaled and upgraded separately
- Recursive crawl controls with domain and expansion limits
- Optional OpenAI-compatible LLM filtering before indexing
- Simple Docker Compose deployment for the main stack

## Repository Layout

- `apps/web`: React SPA for `/`, `/dev`, and `/console`
- `services/control-api`: admin, developer, rules, jobs, and documents management
- `services/query-api`: public search, suggest, and developer search
- `services/task-api`: crawler claim/report/heartbeat entrypoint and task-plane write side
- `services/scheduler`: rule expansion, timeout recovery, retries, and recrawl scheduling
- `services/projector`: staged ingest recovery and projection runner
- `services/blob-storage`: local blob storage HTTP service
- `services/bootstrap`: one-shot migration and search bootstrap entrypoint
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

Main service data is persisted under `./data` and will be created automatically by Docker when the stack starts. `bootstrap` runs automatically as part of the main stack and initializes migrations, search aliases, default system config, legacy blob backfill, and PostgreSQL-to-OpenSearch reindex when the current aliases are empty.

Legacy developer JSON stores are no longer auto-imported during startup. If you still have the old `dev_auth_store.json` and `developer_store.json`, run `findverse-control-api migrate-legacy --dev-auth-store <path> --developer-store <path>` before starting the new stack. If `--blob-storage-url` or `FINDVERSE_BLOB_STORAGE_URL` is provided, the command also backfills legacy document/result blobs; otherwise `bootstrap` will complete that backfill when the stack starts.

## Crawler Worker

Set one shared crawler auth key in `/console -> Settings`, then install or update a crawler worker directly from GitHub:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --max-jobs 16 --skip-browser-install
```

The installer supports both `x86_64/amd64` and `aarch64/arm64` Linux hosts. It always downloads the latest GitHub release unless you pass `--version <tag>`. The first install auto-generates `crawler_id` locally and writes it into `/etc/findverse-crawler/crawler.env`. Re-running the same command updates the node in place and reuses the saved id.

To pin a specific release during rollout:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --version v0.0.15 --max-jobs 16 --skip-browser-install
```

## Development Notes

- Main stack deployment is `docker compose up -d --build`
- Crawler traffic enters through `web -> task-api`; admin and developer traffic enter through `web -> control-api`
- Crawler nodes are intended to run as host services, not inside the main production compose stack
- Current CI only runs regular validation from `.github/workflows/_validate.yml`
- Release artifacts are published for both `x86_64` and `arm64`, and `install-crawler.sh` consumes those release packages directly

## Documentation

- Deployment and operations: [docs/deployment.md](docs/deployment.md)
- Architecture overview: [docs/architecture.md](docs/architecture.md)
