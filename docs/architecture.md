# FindVerse Architecture

## Runtime split

- `query-api`: public search traffic and developer search traffic
- `control-api`: admin UI, developer portal, rules, documents, and control-plane management
- `task-api`: crawler claim/report/heartbeat entrypoint and task-plane write side
- `scheduler`: maintenance loop, staged ingest projection, stale recovery, and recrawl scheduling
- `crawler`: fetch, parse, discover, report
- `web`: single SPA that proxies search routes to `query-api`, crawler internal routes to `task-api`, and everything else to `control-api`

## Storage split

- `PostgreSQL`: users, sessions, API keys, crawler workers, crawl rules, crawl jobs, crawl events, indexed document metadata
- `Redis/Valkey`: basic rate limiting
- `OpenSearch`: search index and suggestions

## Search flow

1. `crawler` fetches and parses pages.
2. `task-api` persists the report into staged ingest storage.
3. `scheduler` projects staged results into `succeeded`, `failed`, `blocked`, `dead_letter`, or re-queued `queued`.
4. Successful documents are written to PostgreSQL metadata tables and indexed into OpenSearch.
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
- Keep control-plane bootstrap imports and admin/developer seed logic inside `control-api`.
- Keep crawler task traffic out of `control-api`; it belongs to `task-api`.
- Keep maintenance and staged ingest projection inside `scheduler`.
- Keep local filesystem blob storage as the only blob backend for now.
- Do not add Kafka, remote object storage, or learning-to-rank yet.
- Prioritize crawler correctness, management UX, and a clean search/control split.
