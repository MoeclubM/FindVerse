# Deployment And Operations

## Services

- `postgres`
- `valkey`
- `opensearch`
- `control-api`
- `query-api`
- `web`
- `crawler-worker` via the `crawler` profile

## What this document covers

- Docker deployment of the main stack
- Docker or script deployment of crawler workers
- Smoke testing and Playwright validation
- Release tagging and GitHub Actions automation
- API entry points and proxy routing

## Docker Compose

`docker-compose.yml` exposes:

- `postgres` on `5432`
- `valkey` on `6379`
- `opensearch` on `9200`
- `control-api` on `8080`
- `query-api` on `8081`
- `web` on `3000`

Start the stack:

Windows:

```powershell
.\scripts\deploy-stack.ps1 -Rebuild
```

Linux or WSL:

```bash
./scripts/deploy-stack.sh --rebuild
```

Start the stack with the bundled crawler worker:

Windows:

```powershell
.\scripts\deploy-stack.ps1 -Rebuild -WithCrawler -CrawlerJoinKey change-me
```

Linux or WSL:

```bash
./scripts/deploy-stack.sh --rebuild --with-crawler --crawler-join-key change-me
```

## Local run

```bash
docker compose up -d postgres valkey opensearch
cargo run -p findverse-control-api
cargo run -p findverse-query-api
npm run dev:web
```

## Crawler deployment

### Scripted local worker

Windows:

```powershell
.\scripts\crawler-setup.ps1 -Server http://127.0.0.1:3000/api -JoinKey change-me -Start
```

Linux or WSL:

```bash
./scripts/crawler-setup.sh --server http://127.0.0.1:3000/api --join-key change-me --start
```

The crawler setup scripts:

- auto-register via `/internal/crawlers/join`
- write `.env.crawler`
- start `findverse-crawler` or `cargo run`
- support `concurrency`, `max-jobs`, `poll-interval-secs`, `allowed-domains`, and `proxy`

### Docker worker

`crawler-worker` is available through the compose `crawler` profile. It uses:

- `FINDVERSE_CRAWLER_SERVER`
- `FINDVERSE_CRAWLER_JOIN_KEY`
- `FINDVERSE_CRAWLER_MAX_JOBS`
- `FINDVERSE_CRAWLER_POLL_INTERVAL_SECS`
- `FINDVERSE_CRAWLER_CONCURRENCY`
- `FINDVERSE_CRAWLER_ALLOWED_DOMAINS`
- `FINDVERSE_CRAWLER_PROXY`

## Smoke tests

Windows:

```powershell
.\scripts\smoke-test.ps1 -ApiBaseUrl http://127.0.0.1:3000/api -ControlApiBaseUrl http://127.0.0.1:8080 -QueryApiBaseUrl http://127.0.0.1:8081
```

Linux or WSL:

```bash
./scripts/smoke-test.sh --api-base-url http://127.0.0.1:3000/api --control-api-base-url http://127.0.0.1:8080 --query-api-base-url http://127.0.0.1:8081
```

With Playwright:

```bash
./scripts/smoke-test.sh --api-base-url http://127.0.0.1:3000/api --run-playwright
```

## Release pipeline

`.github/workflows/release.yml` runs on tag push `v*`.

It:

- validates Rust tests and web typecheck
- builds release binaries for `control-api`, `query-api`, and `crawler`
- creates a GitHub Release with binary artifacts
- publishes GHCR images for `control-api`, `query-api`, `crawler`, and `web`

Release example:

```bash
git tag v0.1.0
git push origin v0.1.0
```

## API highlights

- Query API:
  - `GET /healthz`
  - `GET /readyz`
  - `GET /v1/search`
  - `GET /v1/suggest`
  - `GET /v1/developer/search`
- Control API:
  - `POST /v1/dev/register`
  - `POST /v1/dev/login`
  - `GET /v1/dev/me`
  - `POST /v1/dev/logout`
  - `GET /v1/dev/keys`
  - `POST /v1/dev/keys`
  - `DELETE /v1/dev/keys/:id`
  - `POST /v1/admin/session/login`
  - `GET /v1/admin/session/me`
  - `POST /v1/admin/session/logout`
  - `GET /v1/admin/developers`
  - `PATCH /v1/admin/developers/:user_id`
  - `GET /v1/admin/developers/:user_id/keys`
  - `POST /v1/admin/developers/:user_id/keys`
  - `DELETE /v1/admin/developers/:user_id/keys/:key_id`
  - `GET /v1/admin/crawl/overview`
  - `POST /v1/admin/frontier/seed`
  - `POST /v1/admin/crawl/rules`
  - `PATCH /v1/admin/crawl/rules/:id`
  - `DELETE /v1/admin/crawl/rules/:id`
  - `GET /v1/admin/crawl/jobs`
  - `GET /v1/admin/crawl/jobs/stats`
  - `POST /v1/admin/crawl/jobs/retry`
  - `DELETE /v1/admin/crawl/jobs/completed`
  - `GET /v1/admin/documents`
  - `DELETE /v1/admin/documents/:id`
  - `POST /v1/admin/documents/purge-site`
  - `GET /v1/admin/crawler-join-key`
  - `PUT /v1/admin/crawler-join-key`
  - `POST /internal/crawlers/join`
  - `POST /internal/crawlers/claim`
  - `POST /internal/crawlers/report`

## Search proxying

`apps/web/nginx.conf` forwards:

- `/api/v1/search`
- `/api/v1/suggest`
- `/api/v1/developer/search`

to `query-api`, and forwards the rest of `/api/*` to `control-api`.

The nginx config now uses Docker DNS re-resolution so container IP changes do not leave stale upstreams behind after service rebuilds.
