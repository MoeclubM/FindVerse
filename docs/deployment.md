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

There are now two auth layers handled by the Rust API:

- Console admin auth via local username/password:
  - `FINDVERSE_LOCAL_ADMIN_USERNAME`
  - `FINDVERSE_LOCAL_ADMIN_PASSWORD`
- Developer self-service auth + sessions:
  - `FINDVERSE_DEV_AUTH_STORE`

Developers sign in at `/dev`, create their own `fvk_` API keys, and those keys are then used for protected `/v1/developer/search` access, optional browser key mode, and crawler auto-registration.

## Distributed crawler flow

### Prerequisites

- The FindVerse API must be running and reachable.
- Admin credentials (`FINDVERSE_LOCAL_ADMIN_USERNAME` / `FINDVERSE_LOCAL_ADMIN_PASSWORD`) must be set.
- At least one developer account must exist in `/dev`.
- The crawler worker needs a valid developer `fvk_` API key.

### Step 1: Create a developer API key

1. Open the developer portal at `/dev`.
2. Register or sign in with a developer account.
3. Create an API key and save the raw `fvk_...` token — it is shown only once.
4. Use this same key for protected `/v1/developer/search`, crawler worker registration, or the optional developer-key mode in the browser UI.

### Step 2a: Run a crawler worker locally with auto-registration

```bash
cargo run -p findverse-crawler -- worker \
  --server http://127.0.0.1:8080 \
  --api-key YOUR_DEVELOPER_API_KEY
```

On startup the worker calls `/internal/crawlers/hello`, gets a server-assigned crawler ID, and then starts claiming/reporting jobs.

### Step 2b: Run via Docker Compose

Set the developer API key in your `.env` file:

```env
FINDVERSE_CRAWLER_API_KEY=YOUR_DEVELOPER_API_KEY
```

Then start the crawler profile:

```bash
docker compose --profile crawler up -d
```

### Step 2c: Manual crawler credentials still work

If you want static credentials instead of auto-registration, an admin can still create a crawler in the Console **Workers** tab and run:

```bash
cargo run -p findverse-crawler -- worker \
  --server http://127.0.0.1:8080 \
  --crawler-id YOUR_CRAWLER_ID \
  --crawler-key YOUR_CRAWLER_KEY
```

### Automatic maintenance and stale-job recovery

The API now owns crawl scheduling and recovery:

- enabled crawl rules are evaluated by a background maintenance loop in the API
- newly created enabled rules are seeded immediately
- stale in-flight jobs are requeued automatically after `FINDVERSE_CRAWLER_CLAIM_TIMEOUT_SECS`
- recent automatic enqueue/requeue activity is visible in Console → Overview events

### Step 3: Seed URLs

1. In the Console, click the **Crawl Tasks** tab.
2. Under **Manual crawl**, paste one URL per line into the text area.
3. Set the desired max depth (default: 2) and click **Queue**.

### Step 4: Manage developers and quotas

1. In the Console, open the **Users** tab.
2. Review registered developers.
3. Enable or disable access per account.
4. Adjust per-developer QPS and daily quotas.

### Step 5: Verify

1. Click the **Overview** tab.
2. Watch **In flight** drop to 0 and **Indexed docs** increase.
3. Switch to the **Workers** tab to confirm the worker shows `claimed` and `reported` counts.
4. Open `/dev` and confirm the developer account shows the expected key inventory and quota counters.

### Troubleshooting

- **401 on `/dev` actions**: The developer session is missing or expired. Sign in again.
- **401 on worker start**: The developer API key or manual crawler credential is invalid.
- **429 on protected search**: The developer hit the configured QPS or daily quota on `/v1/developer/search`. Raise limits in Console → Users.
- **Worker runs but no documents appear**: Check that URLs were seeded in the Crawl Tasks tab. The frontier may be empty.
- **`FINDVERSE_CRAWLER_API_KEY` not set**: Ensure the `.env` file is in the project root and Docker Compose can read it.
- **In-flight jobs stuck > 0**: A worker may have crashed mid-job. The API will requeue stale jobs after `FINDVERSE_CRAWLER_CLAIM_TIMEOUT_SECS`; inspect recent events in Console → Overview and worker logs.

Workers auto-register through `/internal/crawlers/hello`, claim jobs from `/internal/crawlers/claim`, and submit parsed pages to `/internal/crawlers/report`.

## Crawler join key (no admin credentials needed)

For deploying crawlers to untrusted machines where you don't want to share admin or developer credentials:

1. **Set a join key** — either via `FINDVERSE_CRAWLER_JOIN_KEY` env var on the API server, or in Console → Settings → Crawler join key.

2. **Give the join key to crawler operators.** They can self-register without admin/developer accounts.

3. **Run the setup script:**

   ```bash
   # Unix
   ./scripts/crawler-setup.sh --server https://api.example.com --join-key <KEY> --start

   # Windows
   .\scripts\crawler-setup.ps1 -Server https://api.example.com -JoinKey <KEY> -Start
   ```

   The script calls `POST /internal/crawlers/join`, gets a `crawler_id` + `crawler_key`, caches them in `.env.crawler`, and optionally starts the worker.

4. **Or run the crawler directly:**

   ```bash
   cargo run -p findverse-crawler -- worker \
     --server http://127.0.0.1:8080 \
     --join-key YOUR_JOIN_KEY
   ```

5. **Rotate the join key** in Console → Settings anytime. Existing crawlers keep working (they already have their own `fvc_*` credentials). Only new registrations need the updated key.

### Join key endpoints

- `POST /internal/crawlers/join` — public, accepts `{ "join_key": "...", "name": "..." }`, returns `{ "crawler_id", "crawler_key", "name" }`
- `GET /v1/admin/crawler-join-key` — admin, returns current join key
- `PUT /v1/admin/crawler-join-key` — admin, sets/rotates/clears join key

## Crawler production features

The crawler binary (`findverse-crawler worker`) now includes:

- **robots.txt compliance** — fetches and caches robots.txt per host, respects Disallow/Allow/Crawl-delay
- **Domain-scoped crawling** — `--allowed-domains example.com,docs.example.com` restricts link discovery
- **Concurrent fetching** — `--concurrency 4` processes multiple jobs in parallel with per-domain rate limiting
- **Anti-bot resilience** — cookie jar, exponential backoff on 429/5xx, Retry-After respect, rotating user-agents
- **Language detection** — uses `whatlang` crate instead of hardcoding "unknown"
- **Better content extraction** — strips script/style/nav/footer before body extraction, falls back to body text when no meta description
- **Proxy support** — `--proxy http://proxy:8080` for optional HTTP proxy

## Storage model today

- `bootstrap_documents.json`: indexed documents
- `developer_store.json`: API keys, per-developer quotas, and usage counters
- `crawler_store.json`: crawler credentials, auto-registered crawler records, crawl rules, frontier, in-flight jobs, and crawl events
- `dev_auth_store.json`: developer accounts and active developer sessions

These are all file-backed now and can be replaced independently.

## Storage scalability assessment

### Current characteristics

| Aspect | Implementation | Limitation |
|---|---|---|
| **Search index** | `Vec<IndexedDocument>` in memory, JSON file on disk | O(n) linear scan per query; full file rewrite on every upsert |
| **Scoring** | Substring `contains()` match with fixed weights (title 6.0, snippet 2.5, body 1.6, url 1.0) | No TF-IDF, BM25, stemming, stop words, or phrase matching |
| **Developer store** | `HashMap<String, DeveloperRecord>` in memory, JSON file on disk | Full rewrite per API key validation that increments counters |
| **Crawler store** | Single `CrawlerStoreState` struct, JSON file on disk | Full rewrite per job claim/report; `VecDeque` frontier is O(n) linear scan per claim |
| **Suggestions** | Sorted `Vec<String>`, prefix `starts_with` match | No fuzzy/typo tolerance |
| **Persistence** | Atomic write (temp → rename) inside write lock | Single-process only; no multi-instance |
| **Concurrency** | `RwLock` / `AsyncRwLock` per store | Single-process only; no multi-instance |

### Scale boundaries

- **<10K documents, <100 QPS**: current design works well as a local/small-team search appliance
- **~50K+ documents**: linear-scan search becomes perceptibly slow (>100ms per query)
- **~1K+ frontier jobs**: O(n) `take_frontier_job()` scan per claim becomes expensive
- **High write throughput**: full JSON rewrite on every counter increment or job claim bottlenecks on I/O

### Architecture extensibility

The separation is clean. Each store (`SearchIndex`, `DeveloperStore`, `CrawlerStore`) is behind a module interface. Swapping the backend from JSON files to SQLite/PostgreSQL requires changing only the inner storage methods without touching HTTP handlers or business logic.

### Recommended upgrade path

1. **Current (this version)**: Atomic file writes (write to temp → rename) prevent corruption on crash
2. **~10K docs**: Replace `SearchIndex` backend with SQLite FTS5 for proper full-text search with stemming and ranking
3. **~50K docs**: Move `CrawlerStore` to SQLite for efficient frontier/in-flight management; move `DeveloperStore` for atomic counter updates
4. **Scale-out**: If multi-instance is needed, move to PostgreSQL. The HTTP handler layer doesn't change

## Compose

The root `docker-compose.yml` exposes:

- `api`
- `web`
- `crawler-worker` as an optional profile
- supporting infra services for future storage and analytics integration
