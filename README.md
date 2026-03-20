# FindVerse

FindVerse is now split into three runtime modules and one docs module:

- `apps/web`: minimal static SPA built with Vite + React
- `services/api`: Rust `axum` API for search, admin auth, crawler control, and document management
- `services/crawler`: Rust crawler worker and offline crawl tooling
- `docs`: architecture and deployment notes

## Current product shape

- Home page: one flat search bar and search results
- Console: `/console`
- Console features:
  - local username/password login
  - API key management
  - crawler credential management
  - manual frontier seeding
  - automatic crawl rules
  - crawl event history
  - indexed document listing and deletion
  - purge indexed data by site

## API highlights

- Public:
  - `GET /healthz`
  - `GET /v1/search`
  - `GET /v1/suggest`
- Admin:
  - `POST /v1/admin/session/login`
  - `GET /v1/admin/session/me`
  - `POST /v1/admin/session/logout`
  - `GET /v1/admin/usage`
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
   - console: `http://127.0.0.1:3000/console`

5. Set up a crawler worker — see [docs/deployment.md](docs/deployment.md) for the complete guide covering credential creation, local and Docker Compose workflows, URL seeding, and verification.

6. Or run the containers:

   ```powershell
   podman compose up --build
   ```

## Environment

- `FINDVERSE_API_BIND`: API bind address
- `FINDVERSE_INDEX_PATH`: indexed document store path
- `FINDVERSE_DEVELOPER_STORE`: API key store path
- `FINDVERSE_CRAWLER_STORE`: crawler store path
- `FINDVERSE_FRONTEND_ORIGIN`: allowed frontend origin for direct browser access
- `FINDVERSE_LOCAL_ADMIN_USERNAME`: local admin username
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`: local admin password
- `FINDVERSE_CRAWLER_ID`: crawler worker id for Docker deployments
- `FINDVERSE_CRAWLER_KEY`: crawler worker key for Docker deployments

## Storage and crawler extensibility

- Search data, API keys, crawler state, rules, and crawl history are currently file-backed JSON stores.
- The API already keeps search indexing and crawler control as separate modules, so replacing file storage with PostgreSQL, OpenSearch, Redpanda, or object storage is a storage-layer change, not a frontend rewrite.
- Worker-to-server integration stays on `x-crawler-id + Bearer key`, so scaling crawler workers horizontally only needs shared API reachability and shared backend storage.
