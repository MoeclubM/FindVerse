# FindVerse Architecture Notes

This repository now implements a flatter V1 slice focused on three concerns: a minimal search UI, a console control plane, and a crawler service.

## Current shape

- Query plane:
  - Rust `axum` API exposes public search endpoints plus admin endpoints for crawler and document control.
  - Search data is file-backed today, but isolated behind a `SearchIndex` module so the storage backend can change later.
- Experience plane:
  - A static Vite SPA serves two routes: `/` for search and `/console` for crawler/data administration.
  - The frontend does not own auth anymore; it calls API login directly and stores a local admin token in browser storage.
- Crawl plane:
  - Rust crawler service covers seed expansion, distributed worker mode, and static bootstrap index generation.
  - The API owns frontier state, crawl rules, crawl history, crawler credentials, and document ingestion.

## Planned replacement points

- file-backed `SearchIndex` -> OpenSearch-backed repository
- file-backed `DeveloperStore` -> PostgreSQL + Valkey-backed API key control plane
- file-backed `CrawlerStore` -> Redpanda + PostgreSQL + object storage crawler flow
- static ranking -> blended lexical relevance plus link quality/freshness signals

## Beta defaults

- Query API accepts anonymous traffic and optional bearer keys for usage tracking.
- Admin API uses local username/password login and opaque admin session tokens.
- Automatic crawl rules are scheduler-lite: the API enqueues due seeds during admin overview reads and crawler claim cycles.
- Search ranking is lexical and deterministic.
- JavaScript rendering is not part of the default crawl path.
