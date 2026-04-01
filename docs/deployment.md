# Deployment And Operations

## Main Stack

Main services:

- `postgres`
- `valkey`
- `opensearch`
- `control-api`
- `query-api`
- `task-api`
- `scheduler`
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

Main service data is persisted under:

- `./data/postgres`
- `./data/valkey`
- `./data/opensearch`

After the first successful admin login, set `FINDVERSE_BOOTSTRAP_ADMIN_ENABLED=false` in `.env` and run the same compose command again.

## Crawler Workers

Crawler workers are host services, not Docker services.

Set one shared crawler auth key in `/console -> Settings`, then install or update a node with the same command:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --channel release --concurrency 16 --skip-browser-install
```

What the installer writes:

- binary: `/opt/findverse-crawler/findverse-crawler`
- launcher: `/opt/findverse-crawler/run-crawler.sh`
- config: `/etc/findverse-crawler/crawler.env`
- service: `findverse-crawler.service`

The first install auto-generates `CRAWLER_ID` locally and writes it into `/etc/findverse-crawler/crawler.env`. Re-running the same command updates the binary and restarts the service in place. Existing `SERVER`, `CRAWLER_ID`, and `CRAWLER_KEY` are reused from the env file if you omit them later.

Use the development channel only when you explicitly want the latest successful CI crawler build:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo env GITHUB_TOKEN=<TOKEN> bash -s -- --server https://search.example.com/api --crawler-key "<crawler-key>" --channel dev --concurrency 16 --skip-browser-install
```

`--channel release` does not need a token. `--channel dev` does, because GitHub Actions artifact downloads require authenticated API access.

## Online Deployment

Recommended topology:

- one Linux host for the main Docker stack
- one or more Linux hosts for crawler workers
- only `web` exposed publicly
- `postgres`, `valkey`, `opensearch`, `control-api`, `query-api`, and `task-api` kept on private bind addresses

Main stack:

```bash
git pull --ff-only
docker compose up -d --build
```

Crawler nodes:

```bash
curl -fsSL https://raw.githubusercontent.com/MoeclubM/FindVerse/main/scripts/install-crawler.sh | sudo bash -s -- --channel release
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
rm -rf data/postgres data/valkey data/opensearch
```

## Release Pipeline

`.github/workflows/release.yml` runs on tag push `v*`.

It:

- validates Rust tests and web typecheck
- builds Linux release binaries for `control-api`, `query-api`, `task-api`, `scheduler`, and `crawler`
- creates a GitHub Release with binary artifacts

Release example:

```bash
git tag v0.1.0
git push origin v0.1.0
```
