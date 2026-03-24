# FindVerse Architecture

## Runtime split

- `query-api`: public search traffic and developer search traffic
- `control-api`: admin UI, developer portal, crawler control, scheduling, document management
- `crawler-worker`: fetch, parse, discover, report
- `web`: single SPA that proxies search routes to `query-api` and everything else to `control-api`

## Storage split

- `PostgreSQL`: users, sessions, API keys, crawler workers, crawl rules, crawl jobs, crawl events, indexed document metadata
- `Redis/Valkey`: basic rate limiting
- `OpenSearch`: search index and suggestions

## Search flow

1. `crawler-worker` fetches and parses pages.
2. `control-api` classifies the report into `succeeded`, `failed`, `blocked`, `dead_letter`, or re-queued `queued`.
3. Successful documents are written to PostgreSQL metadata tables and indexed into OpenSearch.
4. `query-api` serves `/v1/search` and `/v1/suggest` from OpenSearch only.

## Crawl flow

1. Seeds and rules create `queued` jobs in PostgreSQL.
2. Workers join through `/internal/crawlers/join`.
3. Workers claim jobs through `/internal/crawlers/claim`; claim increments `attempt_count`.
4. Workers report results through `/internal/crawlers/report`.
5. Retryable failures are re-queued with `next_retry_at`.
6. Non-retryable failures become `failed` or `blocked`.
7. Retry exhaustion becomes `dead_letter`.

## Design defaults for this phase

- Keep authentication simple.
- Keep scheduling inside `control-api`.
- Keep bootstrap imports and admin/developer seed logic inside `control-api`; `query-api` should stay read-only apart from quota tracking and rate limiting.
- Do not add JS rendering, Kafka, object storage, or learning-to-rank yet.
- Prioritize crawler correctness, management UX, and a clean search/control split.
