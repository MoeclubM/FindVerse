# FindVerse Architecture Notes

This repository implements a practical V1 slice of the full FindVerse plan with a deliberately simple split between web, API, crawler, and docs.

## Current shape

- Query plane:
  - Rust `axum` API exposes search, suggest, and developer key endpoints.
  - Search data is loaded through a repository abstraction so the in-memory JSON bootstrap store can be replaced by OpenSearch later.
- Experience plane:
  - Next.js app consumes the same REST API as external developers.
  - Anonymous search is public; the developer portal uses local username/password auth.
- Crawl plane:
  - Rust crawler service covers seed expansion, distributed worker mode, and static bootstrap index generation.
  - Output artifacts are file-based today, matching the eventual `frontier -> fetch -> parse -> index` flow from the plan.

## Planned replacement points

- `JsonSearchStore` -> OpenSearch-backed repository
- `FileDeveloperStore` -> PostgreSQL + Valkey-backed developer control plane
- file frontier and raw documents -> Redpanda + MinIO crawler flow
- static ranking -> blended lexical relevance plus link quality/freshness signals

## Beta defaults

- Query API accepts anonymous traffic for the web app and optional bearer keys for developer usage tracking.
- Search ranking is lexical and deterministic.
- JavaScript rendering is not part of the default crawl path.
