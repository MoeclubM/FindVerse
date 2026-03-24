# FindVerse

FindVerse is a development-stage general search system built for simple deployment and long-term extensibility. It separates the public query path, the control plane, and crawler workers so crawling, indexing, search, and management can evolve independently without turning the project into an over-complicated platform too early.

## Overview

- Public search, suggest, and developer search APIs
- Admin console for developers, crawl rules, crawl jobs, workers, and indexed documents
- Independent crawler workers that can be deployed with Docker or local scripts
- OpenSearch for search, PostgreSQL for control-plane metadata, and Valkey for short-lived coordination state

## Runtime modules

- `apps/web`: React SPA for `/`, `/dev`, and `/console`
- `services/control-api`: admin, developer, crawler control, frontier, jobs, rules, and documents
- `services/query-api`: public search, suggest, and developer search
- `services/crawler`: crawler worker and local crawl tooling
- `services/api`: shared backend library used by the split APIs

## Design goals

- Keep the default deployment simple enough for a single-machine development stack
- Preserve explicit boundaries between crawling, indexing, querying, and management
- Make crawler, indexing, and query pipelines easy to extend without rewriting the whole system

## Documentation

- Deployment, crawler setup, smoke tests, and release flow: [docs/deployment.md](docs/deployment.md)
- Architecture and service boundaries: [docs/architecture.md](docs/architecture.md)
