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
- Script deployment of crawler workers for production nodes
- Optional Docker deployment of crawler workers for local smoke coverage
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

`--rebuild` / `-Rebuild` now stops the app containers first, removes the previous project images, and prunes old builder cache before rebuilding, so repeated local builds do not keep filling the disk.

The default path stays intentionally simple: one compose file, one command, local ports exposed for development.
When you need environment-specific deployment without rewriting scripts, both deployment scripts also support:

- `--env-file <path>` / `-EnvFile <path>` to load a dedicated deployment env file
- `--compose-file <path>` / `-ComposeFile <path>` to replace the default compose file when you truly need a custom override

Example with the shared production env file:

Windows:

```powershell
.\scripts\deploy-stack.ps1 -EnvFile .env.production
```

Linux or WSL:

```bash
./scripts/deploy-stack.sh --env-file .env.production
```

Recommended shared deployment path for long-lived development and production:

- keep one `docker-compose.yml` for both local and production paths
- keep `postgres`, `valkey`, `opensearch`, `control-api`, and `query-api` bound to loopback only
- expose only `web` publicly
- run crawler workers outside Docker with `scripts/crawler-setup.sh`

Production bootstrap:

```bash
cp .env.production.example .env.production
```

Edit `.env.production` and set at least:

- `FINDVERSE_IMAGE_PREFIX`
- `FINDVERSE_IMAGE_TAG`
- `FINDVERSE_FRONTEND_ORIGIN`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`
- `FINDVERSE_POSTGRES_PASSWORD`
- `FINDVERSE_CRAWLER_JOIN_KEY`

The same `.env.production` template can be used for long-lived development as well. For a dev machine, change the origin and bind hosts to local values, but keep the file shape the same so the deployment path does not fork.

The production env example points `FINDVERSE_INDEX_PATH`, `FINDVERSE_DEVELOPER_STORE`, and `FINDVERSE_DEV_AUTH_STORE` at empty JSON templates under `config/production/` so the control plane does not import local demo fixture data by accident.

Deploy the production stack:

```bash
./scripts/deploy-prod.sh --env-file .env.production
```

After the first successful admin login, set `FINDVERSE_BOOTSTRAP_ADMIN_ENABLED=false` in `.env.production` and deploy again so subsequent restarts do not keep the bootstrap-admin path enabled.

Start the stack with the bundled crawler worker:

Windows:

```powershell
.\scripts\deploy-stack.ps1 -Rebuild -WithCrawler -CrawlerJoinKey change-me
```

Linux or WSL:

```bash
./scripts/deploy-stack.sh --rebuild --with-crawler --crawler-join-key change-me
```

That bundled Docker crawler is intended for local development and CI smoke coverage. The recommended production topology is main stack in Docker plus crawler nodes installed as host services.

## Local run

```bash
docker compose up -d postgres valkey opensearch
cargo run -p findverse-control-api
cargo run -p findverse-query-api
npm run dev:web
```

## Crawler deployment

### Machine install and update

For production or long-lived WSL nodes, the recommended entrypoint is:

```bash
sudo ./scripts/install-crawler.sh \
  --server https://search.example.com/api \
  --join-key "<join-key>"
```

What it does:

- downloads the crawler binary from GitHub instead of building in the repo
- installs only the crawler binary under `/opt/findverse-crawler`
- writes only one crawler config file under `/etc/findverse-crawler/crawler.env`
- installs or updates one `systemd` service unit
- reuses existing crawler credentials on updates unless you pass `--rejoin`
- cleans up its temporary download directory automatically

Update the machine in place by re-running the same command. If the env file already exists, `--server` and `--join-key` are only needed when you want to change the target server or rotate credentials with `--rejoin`.

Release channel examples:

```bash
sudo ./scripts/install-crawler.sh \
  --server https://search.example.com/api \
  --channel release
```

```bash
sudo ./scripts/install-crawler.sh \
  --server https://search.example.com/api \
  --channel release \
  --version v0.0.2
```

Dev channel example:

```bash
sudo GITHUB_TOKEN=github_pat_xxx ./scripts/install-crawler.sh \
  --server https://search.example.com/api \
  --channel dev
```

`--channel dev` downloads the latest successful CI artifact from `.github/workflows/ci.yml`, so it requires GitHub API authentication.

### Scripted local worker

Windows:

```powershell
.\scripts\crawler-setup.ps1 -Server http://127.0.0.1:3000/api -JoinKey change-me -Start
```

Linux or WSL:

```bash
./scripts/crawler-setup.sh --server http://127.0.0.1:3000/api --join-key change-me --start
```

Install the crawler as a WSL `systemd` service:

```bash
sudo ./scripts/crawler-setup.sh --server http://127.0.0.1:3000/api --join-key change-me --env-file .env.crawler.fvlocal --install-service --service-name findverse-crawler-fvlocal
```

Recommended production crawler node with the lower-level setup script:

```bash
sudo ./scripts/crawler-setup.sh \
  --server https://search.example.com/api \
  --join-key "$FINDVERSE_CRAWLER_JOIN_KEY" \
  --env-file /etc/findverse/crawler.env \
  --install-service \
  --service-name findverse-crawler
```

That path keeps crawler scaling separate from the main search stack, and it avoids rebuilding or redeploying the public web stack whenever you need more crawl capacity.

The crawler setup scripts:

- auto-register via `/internal/crawlers/join`
- write `.env.crawler`
- start `findverse-crawler` or `cargo run`
- optionally install and enable a `systemd` service in WSL
- enable JS rendering by default in current crawler builds
- support `concurrency`, `max-jobs`, `poll-interval-secs`, `allowed-domains`, `proxy`, and optional OpenAI-compatible LLM filter settings

For production crawler nodes, install a local Chromium or Chrome package first so the default JS rendering path can execute when the page heuristics detect a client-rendered shell. If no browser is installed, the worker now logs a warning and falls back to static HTML fetch instead of failing the crawl job.

### Docker worker

`crawler-worker` is available through the compose `crawler` profile. It uses:

- `FINDVERSE_CRAWLER_SERVER`
- `FINDVERSE_CRAWLER_JOIN_KEY`
- `FINDVERSE_CRAWLER_MAX_JOBS`
- `FINDVERSE_CRAWLER_POLL_INTERVAL_SECS`
- `FINDVERSE_CRAWLER_CONCURRENCY`
- `FINDVERSE_CRAWLER_ALLOWED_DOMAINS`
- `FINDVERSE_CRAWLER_PROXY`
- `FINDVERSE_CRAWLER_LLM_BASE_URL`
- `FINDVERSE_CRAWLER_LLM_API_KEY`
- `FINDVERSE_CRAWLER_LLM_MODEL`
- `FINDVERSE_CRAWLER_LLM_MIN_SCORE`
- `FINDVERSE_CRAWLER_LLM_MAX_BODY_CHARS`

Use that profile for local runs, demos, and CI only. Production guidance in this repository assumes non-Docker crawler nodes.

## Recursive crawl controls

Manual seeds and scheduled rules now carry two recursive discovery controls:

- `discovery_scope`: `same_host`, `same_domain`, or `any`
- `max_discovered_urls_per_page`: hard cap for how many discovered links a page can fan out into

The worker still extracts links recursively, but the control plane now applies those limits before enqueueing follow-up jobs, so one broad page does not explode the frontier accidentally.

## Optional LLM page filtering

Recommended production default for now: leave all LLM filter settings unset and prioritize data acquisition quality first. The crawler already works without any LLM settings, and the production env example in this repository intentionally omits them.

Crawler workers can call any OpenAI-compatible `chat/completions` endpoint before indexing a parsed page. The worker asks the model whether the page should:

- be indexed
- continue link discovery
- receive a relevance score and short reason

If the model call fails or returns invalid output, the crawler falls back to the normal non-LLM path instead of failing the crawl job.

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

Validation for both CI and release now comes from the shared reusable workflow `.github/workflows/_validate.yml`, so Rust tests and web typecheck stay aligned instead of drifting between two YAML files.

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
