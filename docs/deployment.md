# Deployment Notes

FindVerse is intentionally split into small deployable units:

- `apps/web`: user-facing frontend
- `services/api`: query API, developer control plane, crawler control plane
- `services/crawler`: crawler worker and offline crawl tools
- `docs`: static project documentation

## Minimal container targets

- `services/api/Dockerfile`: distroless runtime image
- `services/crawler/Dockerfile`: distroless runtime image
- `apps/web/Dockerfile`: static Vite build served by Nginx on Alpine

## Local auth

The Console uses local username/password auth handled by the Rust API.

- `FINDVERSE_LOCAL_ADMIN_USERNAME`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`

## Distributed crawler flow

### Prerequisites

- The FindVerse API must be running and reachable.
- Admin credentials (`FINDVERSE_LOCAL_ADMIN_USERNAME` / `FINDVERSE_LOCAL_ADMIN_PASSWORD`) must be set.

### Step 1: Create a crawler credential

1. Open the Console at `/console` and sign in.
2. Click the **Workers** tab.
3. Enter a name (e.g. `worker-1`) and click **Create**.
4. Copy the `CRAWLER_ID` and `CRAWLER_KEY` from the output block — these are shown only once.

### Step 2a: Run a crawler worker locally

```bash
cargo run -p findverse-crawler -- worker \
  --server http://127.0.0.1:8080 \
  --crawler-id YOUR_CRAWLER_ID \
  --crawler-key YOUR_CRAWLER_KEY
```

### Step 2b: Run via Docker Compose

Set the crawler credentials in your `.env` file:

```env
FINDVERSE_CRAWLER_ID=YOUR_CRAWLER_ID
FINDVERSE_CRAWLER_KEY=YOUR_CRAWLER_KEY
```

Then start the crawler profile:

```bash
docker compose --profile crawler up -d
```

### Step 3: Seed URLs

1. In the Console, click the **Crawl Tasks** tab.
2. Under **Manual crawl**, paste one URL per line into the text area.
3. Set the desired max depth (default: 2) and click **Queue**.

### Step 4: Verify

1. Click the **Overview** tab.
2. Watch **In flight** drop to 0 and **Indexed docs** increase.
3. Switch to the **Workers** tab to confirm the worker shows `claimed` and `reported` counts.

### Troubleshooting

- **401 on worker start**: The crawler ID or key is wrong. Create a new credential in the Console.
- **Worker runs but no documents appear**: Check that URLs were seeded in the Crawl Tasks tab. The frontier may be empty.
- **`FINDVERSE_CRAWLER_ID` not set**: Ensure the `.env` file is in the project root and Docker Compose can read it.
- **In-flight jobs stuck > 0**: A worker may have crashed mid-job. Restart the worker — the API will re-enqueue stuck jobs after a timeout.

Workers claim jobs from `/internal/crawlers/claim` and submit parsed pages to `/internal/crawlers/report`.

## Storage model today

- `bootstrap_documents.json`: indexed documents
- `developer_store.json`: search API keys and usage
- `crawler_store.json`: crawler credentials, crawl rules, frontier, in-flight jobs, and crawl events

These are all file-backed now and can be replaced independently.

## Compose

The root `docker-compose.yml` exposes:

- `api`
- `web`
- `crawler-worker` as an optional profile
- supporting infra services for future storage and analytics integration
