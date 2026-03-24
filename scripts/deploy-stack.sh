#!/usr/bin/env bash
set -euo pipefail

COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-findverse}"
WEB_PORT="${FINDVERSE_WEB_PORT:-3000}"
CONTROL_API_PORT="${FINDVERSE_CONTROL_API_PORT:-8080}"
QUERY_API_PORT="${FINDVERSE_QUERY_API_PORT:-8081}"
POSTGRES_PORT="${FINDVERSE_POSTGRES_PORT:-5432}"
REDIS_PORT="${FINDVERSE_REDIS_PORT:-6379}"
OPENSEARCH_PORT="${FINDVERSE_OPENSEARCH_PORT:-9200}"
ADMIN_USERNAME="${FINDVERSE_LOCAL_ADMIN_USERNAME:-admin}"
ADMIN_PASSWORD="${FINDVERSE_LOCAL_ADMIN_PASSWORD:-change-me}"
CRAWLER_JOIN_KEY="${FINDVERSE_CRAWLER_JOIN_KEY:-}"
CRAWLER_SERVER="${FINDVERSE_CRAWLER_SERVER:-http://control-api:8080}"
CRAWLER_MAX_JOBS="${FINDVERSE_CRAWLER_MAX_JOBS:-10}"
CRAWLER_POLL_INTERVAL_SECS="${FINDVERSE_CRAWLER_POLL_INTERVAL_SECS:-5}"
CRAWLER_CONCURRENCY="${FINDVERSE_CRAWLER_CONCURRENCY:-4}"
CRAWLER_ALLOWED_DOMAINS="${FINDVERSE_CRAWLER_ALLOWED_DOMAINS:-}"
CRAWLER_PROXY="${FINDVERSE_CRAWLER_PROXY:-}"
WITH_CRAWLER=false
REBUILD=false

wait_for_tcp() {
  local name="$1"
  local host="$2"
  local port="$3"
  local attempt
  for attempt in $(seq 1 60); do
    if (echo >"/dev/tcp/$host/$port") >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "$name did not become reachable on $host:$port" >&2
  exit 1
}

wait_for_http() {
  local name="$1"
  local url="$2"
  local attempt
  for attempt in $(seq 1 60); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
  done
  echo "$name did not become ready at $url" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project-name) COMPOSE_PROJECT_NAME="$2"; shift 2 ;;
    --web-port) WEB_PORT="$2"; shift 2 ;;
    --control-api-port) CONTROL_API_PORT="$2"; shift 2 ;;
    --query-api-port) QUERY_API_PORT="$2"; shift 2 ;;
    --postgres-port) POSTGRES_PORT="$2"; shift 2 ;;
    --redis-port) REDIS_PORT="$2"; shift 2 ;;
    --opensearch-port) OPENSEARCH_PORT="$2"; shift 2 ;;
    --admin-username) ADMIN_USERNAME="$2"; shift 2 ;;
    --admin-password) ADMIN_PASSWORD="$2"; shift 2 ;;
    --crawler-join-key) CRAWLER_JOIN_KEY="$2"; shift 2 ;;
    --crawler-server) CRAWLER_SERVER="$2"; shift 2 ;;
    --crawler-max-jobs) CRAWLER_MAX_JOBS="$2"; shift 2 ;;
    --crawler-poll-interval-secs) CRAWLER_POLL_INTERVAL_SECS="$2"; shift 2 ;;
    --crawler-concurrency) CRAWLER_CONCURRENCY="$2"; shift 2 ;;
    --crawler-allowed-domains) CRAWLER_ALLOWED_DOMAINS="$2"; shift 2 ;;
    --crawler-proxy) CRAWLER_PROXY="$2"; shift 2 ;;
    --with-crawler) WITH_CRAWLER=true; shift ;;
    --rebuild) REBUILD=true; shift ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

export COMPOSE_PROJECT_NAME
export FINDVERSE_WEB_PORT="$WEB_PORT"
export FINDVERSE_CONTROL_API_PORT="$CONTROL_API_PORT"
export FINDVERSE_QUERY_API_PORT="$QUERY_API_PORT"
export FINDVERSE_POSTGRES_PORT="$POSTGRES_PORT"
export FINDVERSE_REDIS_PORT="$REDIS_PORT"
export FINDVERSE_OPENSEARCH_PORT="$OPENSEARCH_PORT"
export FINDVERSE_LOCAL_ADMIN_USERNAME="$ADMIN_USERNAME"
export FINDVERSE_LOCAL_ADMIN_PASSWORD="$ADMIN_PASSWORD"
export FINDVERSE_CRAWLER_JOIN_KEY="$CRAWLER_JOIN_KEY"
export FINDVERSE_CRAWLER_SERVER="$CRAWLER_SERVER"
export FINDVERSE_CRAWLER_MAX_JOBS="$CRAWLER_MAX_JOBS"
export FINDVERSE_CRAWLER_POLL_INTERVAL_SECS="$CRAWLER_POLL_INTERVAL_SECS"
export FINDVERSE_CRAWLER_CONCURRENCY="$CRAWLER_CONCURRENCY"
export FINDVERSE_CRAWLER_ALLOWED_DOMAINS="$CRAWLER_ALLOWED_DOMAINS"
export FINDVERSE_CRAWLER_PROXY="$CRAWLER_PROXY"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
cd "$repo_root"

infra_args=(compose up -d postgres valkey opensearch)
app_args=(compose up -d)
if $REBUILD; then
  app_args+=(--build)
fi
app_args+=(control-api query-api web)
if $WITH_CRAWLER; then
  crawler_args=(compose --profile crawler up -d)
  if $REBUILD; then
    crawler_args+=(--build)
  fi
  crawler_args+=(crawler-worker)
fi

docker "${infra_args[@]}"
wait_for_tcp "PostgreSQL" "127.0.0.1" "$POSTGRES_PORT"
wait_for_tcp "Redis" "127.0.0.1" "$REDIS_PORT"
wait_for_http "OpenSearch" "http://127.0.0.1:${OPENSEARCH_PORT}"

docker "${app_args[@]}"
wait_for_http "Control API" "http://127.0.0.1:${CONTROL_API_PORT}/healthz"
wait_for_http "Query API" "http://127.0.0.1:${QUERY_API_PORT}/readyz"

if $WITH_CRAWLER; then
  docker "${crawler_args[@]}"
fi

docker compose ps

echo
echo "Stack is running."
echo "  Web:         http://127.0.0.1:${WEB_PORT}"
echo "  Control API: http://127.0.0.1:${CONTROL_API_PORT}"
echo "  Query API:   http://127.0.0.1:${QUERY_API_PORT}"
echo "  PostgreSQL:  127.0.0.1:${POSTGRES_PORT}"
echo "  Redis:       127.0.0.1:${REDIS_PORT}"
echo "  OpenSearch:  http://127.0.0.1:${OPENSEARCH_PORT}"
if $WITH_CRAWLER; then
  echo "  Crawler:     docker compose profile 'crawler' enabled"
fi
