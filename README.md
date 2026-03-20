# FindVerse

FindVerse is now split into three runtime modules and one docs module:

- `apps/web`: minimal static SPA built with Vite + React
- `services/api`: Rust `axum` API for search, admin auth, crawler control, and document management
- `services/crawler`: Rust crawler worker and offline crawl tooling
- `docs`: architecture and deployment notes

## Current product shape

- Home page: one flat search bar and search results at `/`
  - works immediately as browser search with no stored developer token
  - automatically switches to protected developer-key search when an active `fvk_` key is selected in `/dev`
- Developer portal: `/dev`
  - developer account registration and sign-in
  - self-managed `fvk_` search API keys
  - active search key selection for the web search UI
- Console: `/console`
- Console features:
  - local username/password login
  - dense operational summary with identity, quota, and automation status
  - developer user management (enable/disable + quota edits)
  - admin API key management
  - crawler credential management
  - manual frontier seeding
  - automatic crawl rules
  - crawl event history
  - indexed document listing and deletion
  - purge indexed data by site

## API highlights

- Public:
  - `GET /healthz`
  - `GET /v1/search` (browser-facing search, no API key required)
  - `GET /v1/suggest`
- Protected developer search:
  - `GET /v1/developer/search` (`Authorization: Bearer fvk_*`, usage tracked)
- Developer self-service:
  - `POST /v1/dev/register`
  - `POST /v1/dev/login`
  - `GET /v1/dev/me`
  - `POST /v1/dev/logout`
  - `GET /v1/dev/keys`
  - `POST /v1/dev/keys`
  - `DELETE /v1/dev/keys/:id`
- Admin:
  - `POST /v1/admin/session/login`
  - `GET /v1/admin/session/me`
  - `POST /v1/admin/session/logout`
  - `GET /v1/admin/usage`
  - `GET /v1/admin/developers`
  - `PATCH /v1/admin/developers/:user_id`
  - `POST /v1/admin/api-keys`
  - `DELETE /v1/admin/api-keys/:id`
  - `POST /v1/admin/crawlers`
  - `POST /v1/admin/frontier/seed`
  - `GET /v1/admin/crawl/overview`
  - `POST /v1/admin/crawl/rules`
  - `PATCH /v1/admin/crawl/rules/:id`
  - `DELETE /v1/admin/crawl/rules/:id`
  - `GET /v1/admin/documents`
  - `DELETE /v1/admin/documents/:id`
  - `POST /v1/admin/documents/purge-site`
- Internal crawler:
  - `POST /internal/crawlers/hello`
  - `POST /internal/crawlers/join`
  - `POST /internal/crawlers/claim`
  - `POST /internal/crawlers/report`

## Quick start

1. Install dependencies:

   ```powershell
   npm install
   cargo build --workspace
   ```

2. Start the API:

   ```powershell
   cargo run -p findverse-api
   ```

3. Start the web UI:

   ```powershell
   npm run dev:web
   ```

4. Open:

   - search: `http://127.0.0.1:3000/`
   - developer portal: `http://127.0.0.1:3000/dev`
   - console: `http://127.0.0.1:3000/console`

5. Browser search works directly from `http://127.0.0.1:3000/` with no setup.

6. Create a developer account in `/dev`, generate an `fvk_` API key, and use that key for protected API access via `/v1/developer/search`, crawler auto-registration, or to switch the browser UI into developer-key mode.

7. Set up a crawler worker — see [docs/deployment.md](docs/deployment.md) for the complete guide covering developer registration, API key generation, local and Docker Compose workflows, URL seeding, verification, automatic scheduling, and troubleshooting.

8. Or run the containers:

   ```powershell
   podman compose up --build
   ```

## Environment

- `FINDVERSE_API_BIND`: API bind address
- `FINDVERSE_INDEX_PATH`: indexed document store path
- `FINDVERSE_DEVELOPER_STORE`: API key store path
- `FINDVERSE_CRAWLER_STORE`: crawler store path
- `FINDVERSE_DEV_AUTH_STORE`: developer account + session store path
- `FINDVERSE_FRONTEND_ORIGIN`: allowed frontend origin for direct browser access
- `FINDVERSE_LOCAL_ADMIN_USERNAME`: local admin username
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`: local admin password
- `FINDVERSE_CRAWLER_MAINTENANCE_INTERVAL_SECS`: how often the API runs automatic crawl maintenance
- `FINDVERSE_CRAWLER_CLAIM_TIMEOUT_SECS`: how long an in-flight crawl job can remain claimed before it is requeued
- `FINDVERSE_CRAWLER_JOIN_KEY`: reusable key for external crawlers to self-register without admin credentials
- `FINDVERSE_CRAWLER_API_KEY`: developer API key used by auto-registering crawler workers in Compose

## Storage and crawler extensibility

- Search data, developer accounts/sessions, API keys, crawler state, rules, and crawl history are currently file-backed JSON stores.
- The API already keeps search indexing and crawler control as separate modules, so replacing file storage with PostgreSQL, OpenSearch, Redpanda, or object storage is a storage-layer change, not a frontend rewrite.
- Worker-to-server integration now supports either `x-crawler-id + Bearer crawler key` or developer API-key based auto-registration via `/internal/crawlers/hello`, so scaling crawler workers horizontally only needs shared API reachability and shared backend storage.
- External crawlers can self-register via a **crawler join key** (`FINDVERSE_CRAWLER_JOIN_KEY` or Console → Settings), removing the need to share admin/developer credentials with crawler operators.
- File persistence uses atomic writes (write → temp → rename) for crash safety.
- See [docs/deployment.md](docs/deployment.md) for the complete scalability assessment and recommended upgrade path.
