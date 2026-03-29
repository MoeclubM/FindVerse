#!/usr/bin/env bash
set -euo pipefail

COMPOSE_PROJECT_NAME="${COMPOSE_PROJECT_NAME:-findverse}"
ENV_FILE="${FINDVERSE_ENV_FILE:-}"
COMPOSE_FILES=("${FINDVERSE_COMPOSE_FILE:-docker-compose.yml}")
CUSTOM_COMPOSE_FILES=false
WEB_PORT="${FINDVERSE_WEB_PORT:-3000}"
CONTROL_API_PORT="${FINDVERSE_CONTROL_API_PORT:-8080}"
QUERY_API_PORT="${FINDVERSE_QUERY_API_PORT:-8081}"
POSTGRES_PORT="${FINDVERSE_POSTGRES_PORT:-5432}"
REDIS_PORT="${FINDVERSE_REDIS_PORT:-6379}"
OPENSEARCH_PORT="${FINDVERSE_OPENSEARCH_PORT:-9200}"
ADMIN_USERNAME="${FINDVERSE_LOCAL_ADMIN_USERNAME:-admin}"
ADMIN_PASSWORD="${FINDVERSE_LOCAL_ADMIN_PASSWORD:-change-me}"
CRAWLER_JOIN_KEY="${FINDVERSE_CRAWLER_JOIN_KEY:-change-me}"
CRAWLER_SERVER="${FINDVERSE_CRAWLER_SERVER:-http://control-api:8080}"
CRAWLER_MAX_JOBS="${FINDVERSE_CRAWLER_MAX_JOBS:-16}"
CRAWLER_POLL_INTERVAL_SECS="${FINDVERSE_CRAWLER_POLL_INTERVAL_SECS:-5}"
CRAWLER_CONCURRENCY="${FINDVERSE_CRAWLER_CONCURRENCY:-16}"
CRAWLER_ALLOWED_DOMAINS="${FINDVERSE_CRAWLER_ALLOWED_DOMAINS:-}"
CRAWLER_PROXY="${FINDVERSE_CRAWLER_PROXY:-}"
WITH_CRAWLER=false
REBUILD=false

docker_compose() {
  local args=()
  local compose_file

  if [[ -n "$ENV_FILE" ]]; then
    args+=(--env-file "$ENV_FILE")
  fi

  for compose_file in "${COMPOSE_FILES[@]}"; do
    args+=(-f "$compose_file")
  done

  docker compose "${args[@]}" "$@"
}

prune_rebuild_artifacts() {
  local services=("control-api" "query-api" "web")
  local images=()
  local service

  if $WITH_CRAWLER; then
    services+=("crawler-worker")
  fi

  for service in "${services[@]}"; do
    images+=("${COMPOSE_PROJECT_NAME}-${service}")
  done

  docker_compose stop "${services[@]}" >/dev/null 2>&1 || true
  docker_compose rm -f "${services[@]}" >/dev/null 2>&1 || true
  docker image rm -f "${images[@]}" >/dev/null 2>&1 || true
  docker image prune -f >/dev/null 2>&1 || true
  docker builder prune -f >/dev/null 2>&1 || true
}

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

sync_crawler_join_key() {
  local login_payload admin_token status_code

  login_payload="$(curl -fsS -X POST "http://127.0.0.1:${CONTROL_API_PORT}/v1/admin/session/login" \
    -H "Content-Type: application/json" \
    -d "{\"username\":\"${ADMIN_USERNAME}\",\"password\":\"${ADMIN_PASSWORD}\"}")"
  admin_token="$(printf '%s' "$login_payload" | sed -n 's/.*"token":"\([^"]*\)".*/\1/p')"
  if [[ -z "$admin_token" ]]; then
    echo "failed to obtain admin token while syncing crawler join key" >&2
    exit 1
  fi

  status_code="$(curl -sS -o /dev/null -w "%{http_code}" -X PUT "http://127.0.0.1:${CONTROL_API_PORT}/v1/admin/crawler-join-key" \
    -H "Authorization: Bearer ${admin_token}" \
    -H "Content-Type: application/json" \
    -d "{\"join_key\":\"${CRAWLER_JOIN_KEY}\"}")"
  if [[ "$status_code" != "204" ]]; then
    echo "failed to sync crawler join key via control api (status ${status_code})" >&2
    exit 1
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --project-name) COMPOSE_PROJECT_NAME="$2"; shift 2 ;;
    --env-file) ENV_FILE="$2"; shift 2 ;;
    --compose-file)
      if ! $CUSTOM_COMPOSE_FILES; then
        COMPOSE_FILES=()
        CUSTOM_COMPOSE_FILES=true
      fi
      COMPOSE_FILES+=("$2")
      shift 2
      ;;
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
export FINDVERSE_ENV_FILE="$ENV_FILE"
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

if $REBUILD; then
  prune_rebuild_artifacts
fi

docker_compose up -d postgres valkey opensearch
wait_for_tcp "PostgreSQL" "127.0.0.1" "$POSTGRES_PORT"
wait_for_tcp "Redis" "127.0.0.1" "$REDIS_PORT"
wait_for_http "OpenSearch" "http://127.0.0.1:${OPENSEARCH_PORT}/_cluster/health?wait_for_status=yellow&timeout=60s"

app_args=(up -d)
if $REBUILD; then
  app_args+=(--build)
fi
app_args+=(control-api query-api web)
docker_compose "${app_args[@]}"
wait_for_http "Control API" "http://127.0.0.1:${CONTROL_API_PORT}/healthz"
wait_for_http "Query API" "http://127.0.0.1:${QUERY_API_PORT}/readyz"

if $WITH_CRAWLER; then
  sync_crawler_join_key
  crawler_args=(--profile crawler up -d)
  if $REBUILD; then
    crawler_args+=(--build)
  fi
  crawler_args+=(crawler-worker)
  docker_compose "${crawler_args[@]}"
fi

docker_compose ps

echo
echo "Stack is running."
if [[ -n "$ENV_FILE" ]]; then
  echo "  Env file:    $ENV_FILE"
fi
echo "  Compose:     ${COMPOSE_FILES[*]}"
echo "  Web:         http://127.0.0.1:${WEB_PORT}"
echo "  Control API: http://127.0.0.1:${CONTROL_API_PORT}"
echo "  Query API:   http://127.0.0.1:${QUERY_API_PORT}"
echo "  PostgreSQL:  127.0.0.1:${POSTGRES_PORT}"
echo "  Redis:       127.0.0.1:${REDIS_PORT}"
echo "  OpenSearch:  http://127.0.0.1:${OPENSEARCH_PORT}"
if $WITH_CRAWLER; then
  echo "  Crawler:     docker compose profile 'crawler' enabled"
fi
