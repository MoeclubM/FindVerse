# FindVerse

FindVerse is a development-stage general search system with a split query plane, control plane, and crawler worker.

## Runtime modules

- `apps/web`: React SPA for `/`, `/dev`, `/console`
- `services/api`: shared Rust backend library
- `services/control-api`: admin, developer portal, crawler join/claim/report, rules, jobs, documents
- `services/query-api`: public search, suggest, developer search
- `services/crawler`: crawler worker and offline crawl tooling

## Current shape

- Public search: `GET /v1/search`
- Suggest: `GET /v1/suggest`
- Developer search: `GET /v1/developer/search`
- Developer self-service: `/v1/dev/*`
- Admin and crawler control: `/v1/admin/*` and `/internal/crawlers/*`
- Search data: OpenSearch
- Control-plane metadata: PostgreSQL
- Request limiting: Redis/Valkey

## Quick start

### Docker stack

Windows:

```powershell
.\scripts\deploy-stack.ps1 -Rebuild
```

Linux or WSL:

```bash
./scripts/deploy-stack.sh --rebuild
```

Enable the bundled crawler worker profile:

Windows:

```powershell
.\scripts\deploy-stack.ps1 -Rebuild -WithCrawler -CrawlerJoinKey change-me
```

Linux or WSL:

```bash
./scripts/deploy-stack.sh --rebuild --with-crawler --crawler-join-key change-me
```

Default endpoints:

- search: `http://127.0.0.1:3000/`
- developer portal: `http://127.0.0.1:3000/dev`
- console: `http://127.0.0.1:3000/console`
- control API: `http://127.0.0.1:8080/healthz`
- query API: `http://127.0.0.1:8081/readyz`

### Manual local run

```powershell
docker compose up -d postgres valkey opensearch
cargo run -p findverse-control-api
cargo run -p findverse-query-api
npm run dev:web
```

## Smoke tests

API, developer flow, crawler flow, metadata checks:

Windows:

```powershell
.\scripts\smoke-test.ps1 -ApiBaseUrl http://127.0.0.1:3000/api -ControlApiBaseUrl http://127.0.0.1:8080 -QueryApiBaseUrl http://127.0.0.1:8081
```

Linux or WSL:

```bash
./scripts/smoke-test.sh --api-base-url http://127.0.0.1:3000/api --control-api-base-url http://127.0.0.1:8080 --query-api-base-url http://127.0.0.1:8081
```

Run the smoke test and Playwright E2E together:

Windows:

```powershell
.\scripts\smoke-test.ps1 -ApiBaseUrl http://127.0.0.1:3000/api -RunPlaywright
```

Linux or WSL:

```bash
./scripts/smoke-test.sh --api-base-url http://127.0.0.1:3000/api --run-playwright
```

Direct Playwright run:

```powershell
$env:PLAYWRIGHT_BASE_URL="http://127.0.0.1:3000"
$env:PLAYWRIGHT_API_BASE_URL="http://127.0.0.1:3000/api"
npx playwright test
```

## Crawler deployment

### Local script deployment

Windows:

```powershell
.\scripts\crawler-setup.ps1 -Server http://127.0.0.1:3000/api -JoinKey change-me -Start
```

Linux or WSL:

```bash
./scripts/crawler-setup.sh --server http://127.0.0.1:3000/api --join-key change-me --start
```

The crawler setup script:

- registers through `/internal/crawlers/join`
- caches `CRAWLER_ID` and `CRAWLER_KEY` into `.env.crawler`
- starts `findverse-crawler` or falls back to `cargo run`

Supported worker flags in the setup scripts now include:

- `--concurrency`
- `--max-jobs`
- `--poll-interval-secs`
- `--allowed-domains`
- `--proxy`

### Docker crawler deployment

`docker-compose.yml` now includes `crawler-worker` behind the `crawler` profile. It auto-registers through the join key and keeps polling `control-api`.

Required variables:

- `FINDVERSE_CRAWLER_JOIN_KEY`
- optional: `FINDVERSE_CRAWLER_SERVER`
- optional: `FINDVERSE_CRAWLER_MAX_JOBS`
- optional: `FINDVERSE_CRAWLER_POLL_INTERVAL_SECS`
- optional: `FINDVERSE_CRAWLER_CONCURRENCY`
- optional: `FINDVERSE_CRAWLER_ALLOWED_DOMAINS`
- optional: `FINDVERSE_CRAWLER_PROXY`

## Release automation

Tag push now triggers `.github/workflows/release.yml`.

```powershell
git tag v0.1.0
git push origin v0.1.0
```

The workflow:

- runs `cargo test --workspace`
- runs `npm run typecheck:web`
- builds release binaries for:
  - `findverse-control-api`
  - `findverse-query-api`
  - `findverse-crawler`
- publishes GitHub Release assets
- builds and pushes GHCR images:
  - `ghcr.io/<owner>/findverse-control-api`
  - `ghcr.io/<owner>/findverse-query-api`
  - `ghcr.io/<owner>/findverse-crawler`
  - `ghcr.io/<owner>/findverse-web`

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

## Notes

- Query routing is split at the web proxy layer.
- `/api/v1/search`, `/api/v1/suggest`, and `/api/v1/developer/search` go to `query-api`.
- The rest of `/api/*` goes to `control-api`.
- Crawl jobs use explicit states: `queued`, `claimed`, `succeeded`, `failed`, `blocked`, `dead_letter`.
- Search documents keep canonical URL, host, content hash, parser/schema/index versions, and duplicate linkage.
