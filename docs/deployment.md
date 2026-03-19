# Deployment Notes

FindVerse is intentionally split into small deployable units:

- `apps/web`: user-facing frontend
- `services/api`: query API, developer control plane, crawler control plane
- `services/crawler`: crawler worker and offline crawl tools
- `docs`: static project documentation

## Minimal container targets

- `services/api/Dockerfile`: distroless runtime image
- `services/crawler/Dockerfile`: distroless runtime image
- `apps/web/Dockerfile`: standalone Next.js build on Alpine

## Local auth

The developer portal uses local username/password auth in this repository revision.

- `AUTH_SECRET`
- `FINDVERSE_LOCAL_ADMIN_USERNAME`
- `FINDVERSE_LOCAL_ADMIN_PASSWORD`

## Distributed crawler flow

1. Start `api`.
2. Create a crawler credential from the developer portal.
3. Seed URLs into the frontier.
4. Start one or more `crawler` workers with the issued `crawler id + key`.
5. Workers claim jobs from `/internal/crawlers/claim` and submit parsed pages to `/internal/crawlers/report`.

## Compose

The root `docker-compose.yml` exposes:

- `api`
- `web`
- `crawler-worker` as an optional profile
- supporting infra services for future storage and analytics integration
