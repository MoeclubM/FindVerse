# FindVerse Architecture

## Runtime split

- `bootstrap`: one-shot database migration, legacy blob backfill, alias-safe document reindex, bootstrap admin, and default system config seeding
- `blob-storage`: internal blob read/write service backed by local filesystem only
- `query-api`: public search traffic and developer search traffic
- `control-api`: admin UI, developer portal, rules, documents, and control-plane management
- `task-api`: crawler claim/report/heartbeat entrypoint and task-plane write side
- `scheduler`: rule expansion, stale claim recovery, event trimming, and recrawl scheduling
- `projector`: staged ingest recovery and projection into documents plus OpenSearch
- `crawler`: fetch, parse, discover, report
- `web`: single SPA that proxies search routes to `query-api`, crawler internal routes to `task-api`, and everything else to `control-api`

## Storage split

- `PostgreSQL`: users, sessions, API keys, crawler workers, crawl rules, crawl jobs, crawl events, indexed document metadata
- `Redis/Valkey`: basic rate limiting and Streams-based task message bus with consumer groups
- `OpenSearch`: search aliases, versioned indexes, and suggestions
- `Local blob storage`: crawl result payloads and document text blobs through `blob-storage`

## Search flow

1. `crawler` fetches and parses pages.
2. `task-api` persists the report into staged ingest storage and emits a task-bus stream event.
3. `projector` consumes the Valkey stream through a consumer group, recovers stale ingest work, and projects job outcomes.
4. Successful documents are written to PostgreSQL metadata tables, document blobs, and versioned OpenSearch aliases.
5. `query-api` serves `/v1/search` and `/v1/suggest` from OpenSearch only.

## Crawl flow

1. Seeds and rules create `queued` jobs in PostgreSQL.
2. Admin issues fixed `crawler_id` and `crawler_key` credentials for workers.
3. Workers claim jobs through `task-api /internal/crawlers/claim`; claim increments `attempt_count`.
4. Workers report results through `task-api /internal/crawlers/report`.
5. Retryable failures are re-queued with `next_retry_at`.
6. Non-retryable failures become `failed` or `blocked`.
7. Retry exhaustion becomes `dead_letter`.

## Design defaults for this phase

- Keep authentication simple.
- Keep runtime auth on `argon2id` only. Legacy JSON stores and legacy password hashes are migrated explicitly, not auto-imported at startup.
- Keep crawler task traffic out of `control-api`; it belongs to `task-api`.
- Keep task messages out of `control-api`; they belong to `task-api`, `scheduler`, `projector`, and Valkey.
- Keep local filesystem blob storage as the only blob backend for now, but only behind `blob-storage`.
- Do not add Kafka, remote object storage, or learning-to-rank yet.
- Prioritize crawler correctness, management UX, and a clean search/control split.
