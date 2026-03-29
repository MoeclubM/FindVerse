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

The default path is intentionally simple: one compose file, one `.env`, one command.

Recommended shared deployment path for long-lived development and production:

- keep one `docker-compose.yml` for both local and production paths
- keep `postgres`, `valkey`, `opensearch`, `control-api`, and `query-api` bound to loopback only
- expose only `web` publicly
- run crawler workers outside Docker with the GitHub-hosted `install-crawler.sh` installer

Bootstrap the shared env file:

```bash
cp .env.example .env
```

Edit `.env` and set at least:

- `FINDVERSE_FRONTEND_ORIGIN`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`
- `FINDVERSE_POSTGRES_PASSWORD`
- `FINDVERSE_CRAWLER_JOIN_KEY`

The shared env template points `FINDVERSE_INDEX_PATH`, `FINDVERSE_DEVELOPER_STORE`, and `FINDVERSE_DEV_AUTH_STORE` at empty JSON templates under `config/production/` so the control plane does not import local demo fixture data by accident.

Build and start the main stack:

```bash
docker compose up -d --build
```

Persistent service data now lives under:

- `./data/postgres`
- `./data/valkey`
- `./data/opensearch`

`docker compose down` stops the stack but keeps those directories intact. If you really want a clean reset, stop the stack first and then remove the matching directories under `./data`.

After the first successful admin login, set `FINDVERSE_BOOTSTRAP_ADMIN_ENABLED=false` in `.env` and run the same compose command again so subsequent restarts do not keep the bootstrap-admin path enabled.

Start the stack with the bundled crawler worker:

```bash
docker compose --profile crawler up -d --build
```

That bundled Docker crawler is intended for local development and optional smoke coverage. The recommended production topology is main stack in Docker plus crawler nodes installed as host services.

## Local run

```bash
docker compose up -d postgres valkey opensearch
cargo run -p findverse-control-api
cargo run -p findverse-query-api
npm run dev:web
```

## Crawler deployment

### Machine install and update

For production or long-lived WSL nodes, the only recommended entrypoint is the GitHub-hosted installer:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --join-key "<join-key>" --channel release
```

What it does:

- downloads the installer script from GitHub instead of assuming a local repo checkout
- downloads the crawler binary from GitHub instead of building in the repo
- installs only the crawler binary under `/opt/findverse-crawler`
- writes only one crawler config file under `/etc/findverse-crawler/crawler.env`
- installs or updates one `systemd` service unit
- reuses existing crawler credentials on updates unless you pass `--rejoin`
- cleans up its temporary download directory automatically

Legacy `crawler-setup.sh` and `crawler-setup.ps1` flows have been removed from this repository. Do not build the crawler in-place for production nodes, and do not depend on a repo checkout on the target machine. Use the GitHub installer for both first install and updates.

Public release installs do not need a GitHub token. Only `--channel dev` needs `GITHUB_TOKEN`, because GitHub Actions artifact downloads go through the authenticated API even when the repository itself is public.

The GitHub installer:

- registers the crawler via `/internal/crawlers/join` when needed
- downloads a GitHub release binary or the latest successful CI dev artifact
- keeps crawler scaling separate from the main search stack
- updates the existing `systemd` service in place without leaving extra runtime files behind
- enables JS rendering by default in current crawler builds
- supports `concurrency`, `max-jobs`, `poll-interval-secs`, `allowed-domains`, and `proxy`

For production crawler nodes, install a local Chromium or Chrome package first so the default JS rendering path can execute when the page heuristics detect a client-rendered shell. If no browser is installed, the worker now logs a warning and falls back to static HTML fetch instead of failing the crawl job.

## Online deployment

Recommended production topology:

- one Linux host for the main Docker stack
- one or more Linux hosts for crawler workers
- only `web` exposed publicly
- `postgres`, `valkey`, `opensearch`, `control-api`, and `query-api` kept on private bind addresses

Main stack deployment:

1. Copy the env template and fill in production values.

```bash
cp .env.example .env
```

You must set at least:

- `FINDVERSE_FRONTEND_ORIGIN`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`
- `FINDVERSE_POSTGRES_PASSWORD`
- `FINDVERSE_CRAWLER_JOIN_KEY`

2. Build and start the production stack from the current repo checkout.

```bash
docker compose up -d --build
```

3. Log in once with the bootstrap admin, then turn bootstrap admin back off and deploy again.

```bash
FINDVERSE_BOOTSTRAP_ADMIN_ENABLED=false
docker compose up -d --build
```

4. Install crawler nodes from GitHub.

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --join-key "$FINDVERSE_CRAWLER_JOIN_KEY" --channel release
```

The installer writes the crawler binary to `/opt/findverse-crawler`, the config to `/etc/findverse-crawler/crawler.env`, and a `systemd` unit named `findverse-crawler.service`.

## Upgrade And Maintenance

Main stack updates:

- pull the latest repo changes onto the server
- run `docker compose up -d --build`

Example:

```bash
git pull --ff-only
docker compose up -d --build
```

Crawler node updates:

- stable upgrade: rerun the installer with `--channel release`
- pin a specific release: rerun it with `--channel release --version <tag>`
- follow the latest successful CI build: rerun it with `--channel dev`

Examples:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --channel release
```

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --channel release --version <tag>
```

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo env GITHUB_TOKEN=github_pat_xxx bash -s -- --server https://search.example.com/api --channel dev
```

`--channel dev` downloads the latest successful crawler dev artifact from `.github/workflows/crawler-dev-artifact.yml`, so it requires GitHub API authentication even for a public repo. `--channel release` does not need a token.

Useful maintenance commands on crawler hosts:

```bash
systemctl status findverse-crawler.service
journalctl -u findverse-crawler.service -f
systemctl restart findverse-crawler.service
cat /etc/findverse-crawler/crawler.env
```

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
