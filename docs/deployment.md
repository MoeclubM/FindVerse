# Deployment And Operations

## Main Stack

Main services:

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

Use one compose file and one `.env` file for both online deployment and long-lived development.

Bootstrap the env file:

```bash
cp .env.example .env
```

At minimum, set:

- `FINDVERSE_FRONTEND_ORIGIN`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`
- `FINDVERSE_POSTGRES_PASSWORD`

Start or update the main stack:

```bash
docker compose up -d --build
```

`bootstrap` is a one-shot service. It runs database migrations, seeds default system config, initializes the versioned OpenSearch aliases, backfills legacy document/result blobs into `blob-storage`, reindexes PostgreSQL documents into the current OpenSearch aliases when needed, and creates the bootstrap admin when enabled.

Main service data is persisted under:

- `./data/postgres`
- `./data/valkey`
- `./data/opensearch`
- `./data/blobs`

After the first successful admin login, set `FINDVERSE_BOOTSTRAP_ADMIN_ENABLED=false` in `.env` and run the same compose command again. Runtime services no longer create the bootstrap admin on startup by themselves.

## Legacy Data Migration

Legacy developer JSON stores are no longer imported automatically during startup.

If you still have the old auth store and developer store files, run the migration explicitly before starting the new stack:

```bash
findverse-control-api migrate-legacy \
  --dev-auth-store /path/to/dev_auth_store.json \
  --developer-store /path/to/developer_store.json
```

The command connects to `FINDVERSE_POSTGRES_URL`, imports developer records, rewrites non-argon passwords into fresh temporary passwords, and prints the generated temporary credentials as JSON. If `--blob-storage-url` or `FINDVERSE_BLOB_STORAGE_URL` is provided, it also backfills legacy document/result blobs immediately. Legacy sessions are not migrated, and any missing blob backfill will still be completed by `bootstrap`.

## Crawler Workers

Crawler workers are host services, not Docker services.

Set one shared crawler auth key in `/console -> Settings`, then install or update a node with the same command:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --max-jobs 16 --skip-browser-install
```

The installer supports both `x86_64/amd64` and `aarch64/arm64` Linux hosts. It selects the matching release asset automatically from the current machine architecture and always downloads from GitHub Releases.

`--max-jobs` is a local claim cap for that node. The real claim count is still capped by the concurrency delivered from the control plane heartbeat.

What the installer writes:

- binary: `/opt/findverse-crawler/findverse-crawler`
- launcher: `/opt/findverse-crawler/run-crawler.sh`
- config: `/etc/findverse-crawler/crawler.env`
- service: `findverse-crawler.service`

The first install auto-generates `CRAWLER_ID` locally and writes it into `/etc/findverse-crawler/crawler.env`. Re-running the same command updates the binary and restarts the service in place. Existing `SERVER`, `CRAWLER_ID`, and `CRAWLER_KEY` are reused from the env file if you omit them later.

Pin a specific release during rollout when needed:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --version v0.0.15 --max-jobs 16 --skip-browser-install
```

## Online Deployment

Recommended topology:

- one Linux host for the main Docker stack
- one or more Linux hosts for crawler workers
- only `web` exposed publicly
- `postgres`, `valkey`, `opensearch`, `blob-storage`, `control-api`, `query-api`, and `task-api` kept on private bind addresses

Main stack:

```bash
git pull --ff-only
docker compose up -d --build
```

Crawler nodes:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s --
```

That update command is enough once the node already has `/etc/findverse-crawler/crawler.env`.

## Maintenance

Useful crawler commands:

```bash
systemctl status findverse-crawler.service
journalctl -u findverse-crawler.service -f
systemctl restart findverse-crawler.service
cat /etc/findverse-crawler/crawler.env
```

Clean reset of local compose data:

```bash
docker compose down
rm -rf data/postgres data/valkey data/opensearch data/blobs
```

## Release Pipeline

`.github/workflows/release.yml` runs on tag push `v*`.

It:

- validates Rust tests and web typecheck
- builds Linux release binaries for `bootstrap`, `blob-storage`, `control-api`, `projector`, `query-api`, `task-api`, `scheduler`, and `crawler` on both `x86_64` and `arm64`
- creates a GitHub Release with binary artifacts

Release example:

```bash
git tag v0.1.0
git push origin v0.1.0
```
