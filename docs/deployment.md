# Deployment And Operations

## Control Plane

The control plane includes:

- `postgres`
- `valkey`
- `opensearch`
- `blob-storage`
- `bootstrap`
- `control-api`
- `query-api`
- `task-api`
- `scheduler`
- `projector`
- `web`

Bootstrap the env file:

```bash
cp .env.example .env
```

Set at minimum:

- `FINDVERSE_FRONTEND_ORIGIN`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`
- `FINDVERSE_POSTGRES_PASSWORD`
- `FINDVERSE_CARGO_BUILD_JOBS`

Start or update the control plane:

```bash
docker compose up -d --build
```

On low-memory hosts, prefer:

```bash
COMPOSE_PARALLEL_LIMIT=1 docker compose up -d --build
```

Control-plane data is persisted under:

- `./data/postgres`
- `./data/valkey`
- `./data/opensearch`
- `./data/blobs`

`bootstrap` is a one-shot service. It runs database migrations, seeds default system config, initializes the active OpenSearch aliases, backfills legacy document and result blobs into `blob-storage`, and creates the bootstrap admin when enabled.

After the first successful admin login, set `FINDVERSE_BOOTSTRAP_ADMIN_ENABLED=false` in `.env` and run the same compose command again.

## Legacy Migration

Legacy developer JSON stores are no longer imported automatically during startup.

If you still have the old auth store and developer store files, run the migration before starting the new control plane:

```bash
findverse-control-api migrate-legacy \
  --dev-auth-store /path/to/dev_auth_store.json \
  --developer-store /path/to/developer_store.json
```

The command imports developer records, rewrites non-argon passwords into temporary passwords, and can backfill legacy document and result blobs immediately when `--blob-storage-url` or `FINDVERSE_BLOB_STORAGE_URL` is provided.

## Crawler Nodes

Crawler nodes are host services, not Docker services.

Install or update a node:

```bash
tmp="$(mktemp)" && \
{ curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh -o "$tmp" || \
  curl -fsSL https://gh-proxy.net/https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh -o "$tmp"; } && \
sudo bash "$tmp" -- --server https://search.example.com/api --crawler-key "<crawler-key>" --max-jobs 16 --skip-browser-install; \
status=$?; rm -f "$tmp"; [ $status -eq 0 ]
```

Pin a specific crawler release when needed:

```bash
tmp="$(mktemp)" && \
{ curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh -o "$tmp" || \
  curl -fsSL https://gh-proxy.net/https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh -o "$tmp"; } && \
sudo bash "$tmp" -- --server https://search.example.com/api --crawler-key "<crawler-key>" --version v0.0.15 --max-jobs 16 --skip-browser-install; \
status=$?; rm -f "$tmp"; [ $status -eq 0 ]
```

The installer supports both `x86_64/amd64` and `aarch64/arm64` Linux hosts and downloads the matching crawler release asset automatically.
If direct GitHub access fails, the script retries script and crawler package downloads through `gh-proxy.net` automatically.

It writes:

- binary: `/opt/findverse-crawler/findverse-crawler`
- launcher: `/opt/findverse-crawler/run-crawler.sh`
- config: `/etc/findverse-crawler/crawler.env`
- service: `findverse-crawler.service`

The first install auto-generates `CRAWLER_ID` locally. Re-running the same command updates the binary in place and restarts the service. Existing `SERVER`, `CRAWLER_ID`, and `CRAWLER_KEY` are reused from the env file when omitted.

If a node is still on a crawler version from before the split release packaging change, run `install-crawler.sh` once manually on that machine before using console-triggered remote updates.

`--max-jobs` is only the local claim cap for that node. The actual claim count is still bounded by the concurrency sent from the control-plane heartbeat.

## Recommended Topology

- one Linux host for the control-plane Docker stack
- one or more Linux hosts for crawler workers
- expose only `web` publicly
- keep `postgres`, `valkey`, `opensearch`, `blob-storage`, `control-api`, `query-api`, and `task-api` on private bind addresses

Control-plane update:

```bash
git pull --ff-only
docker compose up -d --build
```

Crawler node update:

```bash
tmp="$(mktemp)" && \
{ curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh -o "$tmp" || \
  curl -fsSL https://gh-proxy.net/https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh -o "$tmp"; } && \
sudo bash "$tmp" --; \
status=$?; rm -f "$tmp"; [ $status -eq 0 ]
```

## Maintenance

Useful crawler commands:

```bash
systemctl status findverse-crawler.service
journalctl -u findverse-crawler.service -f
systemctl restart findverse-crawler.service
cat /etc/findverse-crawler/crawler.env
```

Clean reset of local control-plane data:

```bash
docker compose down
rm -rf data/postgres data/valkey data/opensearch data/blobs
```

## Release Pipeline

`.github/workflows/release.yml` runs on tag push `v*`.

It:

- validates Rust tests and web typecheck
- publishes `findverse-crawler-linux-*` packages for `x86_64` and `arm64`

Control-plane deployment stays source and Docker Compose based. Release assets are only for crawler nodes.
