# FindVerse

FindVerse is split into four clear modules:

- `apps/web`: Next.js frontend
- `services/api`: Rust query and control API
- `services/crawler`: Rust crawler service and worker CLI
- `docs`: standalone architecture and API docs

## What is implemented

- Rust `axum` API with:
  - `GET /healthz`
  - `GET /v1/search`
  - `GET /v1/suggest`
  - `POST /v1/developer/keys`
  - `DELETE /v1/developer/keys/:id`
  - `GET /v1/developer/usage`
  - `GET /v1/developer/crawl/overview`
  - `POST /v1/developer/crawlers`
  - `POST /v1/developer/frontier/seed`
  - `POST /internal/crawlers/claim`
  - `POST /internal/crawlers/report`
- Next.js web app with:
  - public search pages
  - developer portal for search keys
  - crawler credential creation
  - frontier seeding
  - docs page
- Rust crawler service with:
  - seed discovery
  - local fetch and index build commands
  - distributed worker mode using crawler `id + key`
- Dockerfiles for `web`, `api`, and `crawler`
- Compose wiring for app modules plus infra dependencies

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

3. Start the web app:

   ```powershell
   npm run dev:web
   ```

4. Create a crawler in the developer portal, then run a worker:

   ```powershell
   cargo run -p findverse-crawler -- worker --server http://127.0.0.1:8080 --crawler-id YOUR_ID --crawler-key YOUR_KEY
   ```

5. Optional: run the modular Docker stack:

   ```powershell
   docker compose up --build
   ```

## Environment

- `FINDVERSE_API_BIND`: API bind address, default `0.0.0.0:8080`
- `FINDVERSE_INDEX_PATH`: indexed document store path
- `FINDVERSE_DEVELOPER_STORE`: developer key store path
- `FINDVERSE_CRAWLER_STORE`: crawler control store path
- `FINDVERSE_FRONTEND_ORIGIN`: frontend origin for CORS
- `NEXT_PUBLIC_FINDVERSE_API_URL`: frontend API base URL
- `AUTH_SECRET`: web session secret
- `FINDVERSE_LOCAL_ADMIN_USERNAME`: local developer portal username
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`: local developer portal password
- `FINDVERSE_CRAWLER_ID`: crawler worker id for Docker deployments
- `FINDVERSE_CRAWLER_KEY`: crawler worker key for Docker deployments

## Service boundaries

- `apps/web` only handles user-facing pages, auth cookies, and proxy routes.
- `services/api` owns search responses, developer keys, crawler keys, frontier state, and document ingestion.
- `services/crawler` owns fetching, HTML parsing, link extraction, and worker polling behavior.
- `docs` holds architecture and API reference material outside the running services.
