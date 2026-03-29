#!/usr/bin/env bash
set -euo pipefail

API_BASE_URL="http://127.0.0.1:3000/api"
CONTROL_API_BASE_URL=""
QUERY_API_BASE_URL=""
ADMIN_USERNAME="admin"
ADMIN_PASSWORD="change-me"
SEED_URL_TEMPLATE=""
RUN_PLAYWRIGHT=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --api-base-url) API_BASE_URL="${2%/}"; shift 2 ;;
    --control-api-base-url) CONTROL_API_BASE_URL="${2%/}"; shift 2 ;;
    --query-api-base-url) QUERY_API_BASE_URL="${2%/}"; shift 2 ;;
    --admin-username) ADMIN_USERNAME="$2"; shift 2 ;;
    --admin-password) ADMIN_PASSWORD="$2"; shift 2 ;;
    --seed-url) SEED_URL_TEMPLATE="$2"; shift 2 ;;
    --run-playwright) RUN_PLAYWRIGHT=true; shift ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
done

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required" >&2
  exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
  echo "jq is required" >&2
  exit 1
fi

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"

CRAWLER_SERVER="$API_BASE_URL"
if [[ -n "$CONTROL_API_BASE_URL" ]]; then
  CRAWLER_SERVER="$CONTROL_API_BASE_URL"
fi
PUBLIC_BASE_URL="${API_BASE_URL%/api}"

step() {
  echo
  echo "==> $*"
}

assert() {
  if ! eval "$1"; then
    echo "Assertion failed: $2" >&2
    exit 1
  fi
}

json_request() {
  local method="$1"
  local url="$2"
  local body="${3:-}"
  local auth="${4:-}"
  local -a args=(-fsS -X "$method" "$url")
  if [[ -n "$body" ]]; then
    args+=(-H "Content-Type: application/json" -d "$body")
  fi
  if [[ -n "$auth" ]]; then
    args+=(-H "Authorization: Bearer $auth")
  fi
  curl "${args[@]}"
}

status_request() {
  local method="$1"
  local url="$2"
  local body="${3:-}"
  local auth="${4:-}"
  local -a args=(-sS -o /dev/null -w "%{http_code}" -X "$method" "$url")
  if [[ -n "$body" ]]; then
    args+=(-H "Content-Type: application/json" -d "$body")
  fi
  if [[ -n "$auth" ]]; then
    args+=(-H "Authorization: Bearer $auth")
  fi
  curl "${args[@]}"
}

try_probe() {
  local name="$1"
  local url="$2"
  local jq_expr="$3"
  if [[ -z "$url" ]]; then
    return
  fi
  if payload="$(curl -fsS "$url" 2>/dev/null)"; then
    echo "$payload" | jq -e "$jq_expr" >/dev/null
    echo "PASS  $name"
  else
    echo "WARN  skipped $name" >&2
  fi
}

step "Probe health endpoints"
try_probe "control-api healthz" "${CONTROL_API_BASE_URL:+$CONTROL_API_BASE_URL/healthz}" '.status == "ok"'
try_probe "query-api readyz" "${QUERY_API_BASE_URL:+$QUERY_API_BASE_URL/readyz}" '.status == "ready" and .postgres and .redis and .opensearch'

timestamp="$(date +%s)"
developer_username="smoke-dev-$timestamp"
developer_password="smoke-password-123"
if [[ -z "$SEED_URL_TEMPLATE" ]]; then
  SEED_URL_TEMPLATE="$PUBLIC_BASE_URL/smoke-crawler.html?findverse-smoke={timestamp}"
fi
seed_url="${SEED_URL_TEMPLATE//\{timestamp\}/$timestamp}"

step "Search and suggest"
search_payload="$(json_request GET "$API_BASE_URL/v1/search?q=ranking")"
echo "$search_payload" | jq -e '.results | length >= 1' >/dev/null
echo "PASS  GET /v1/search?q=ranking"

filtered_payload="$(json_request GET "$API_BASE_URL/v1/search?q=search&site=example.com&lang=en&freshness=30d")"
echo "$filtered_payload" | jq -e '.query == "search"' >/dev/null
echo "PASS  GET /v1/search?q=search&site=example.com&lang=en&freshness=30d"

suggest_payload="$(json_request GET "$API_BASE_URL/v1/suggest?q=rank")"
echo "$suggest_payload" | jq -e '.suggestions | length >= 1' >/dev/null
echo "PASS  GET /v1/suggest?q=rank"

step "Developer flow"
dev_session="$(json_request POST "$API_BASE_URL/v1/dev/register" "{\"username\":\"$developer_username\",\"password\":\"$developer_password\"}")"
dev_token="$(echo "$dev_session" | jq -r '.token')"
assert "[[ -n \"$dev_token\" && \"$dev_token\" != \"null\" ]]" "developer register returned no token"

created_key="$(json_request POST "$API_BASE_URL/v1/dev/keys" '{"name":"Smoke key"}' "$dev_token")"
api_key="$(echo "$created_key" | jq -r '.token')"
key_id="$(echo "$created_key" | jq -r '.id')"
assert "[[ \"$api_key\" == fvk_* ]]" "developer key token format is invalid"
echo "PASS  developer register and key creation"

developer_search="$(json_request GET "$API_BASE_URL/v1/developer/search?q=ranking" "" "$api_key")"
echo "$developer_search" | jq -e '.results | length >= 1' >/dev/null
echo "PASS  developer bearer search"

revoke_status="$(status_request DELETE "$API_BASE_URL/v1/dev/keys/$key_id" "" "$dev_token")"
assert "[[ \"$revoke_status\" == \"204\" ]]" "developer key revoke did not return 204"

revoked_status="$(status_request GET "$API_BASE_URL/v1/developer/search?q=ranking" "" "$api_key")"
assert "[[ \"$revoked_status\" == \"401\" ]]" "revoked developer key should return 401"
echo "PASS  revoked key is rejected"

step "Admin and crawler flow"
admin_session="$(json_request POST "$API_BASE_URL/v1/admin/session/login" "{\"username\":\"$ADMIN_USERNAME\",\"password\":\"$ADMIN_PASSWORD\"}")"
admin_token="$(echo "$admin_session" | jq -r '.token')"
assert "[[ -n \"$admin_token\" && \"$admin_token\" != \"null\" ]]" "admin login returned no token"

crawler_credentials="$(json_request POST "$API_BASE_URL/v1/admin/crawlers" "{\"name\":\"smoke-crawler-$timestamp\"}" "$admin_token")"
crawler_id="$(echo "$crawler_credentials" | jq -r '.crawler_id')"
crawler_key="$(echo "$crawler_credentials" | jq -r '.crawler_key')"
assert "[[ -n \"$crawler_id\" && \"$crawler_id\" != \"null\" ]]" "crawler creation returned no crawler_id"
assert "[[ -n \"$crawler_key\" && \"$crawler_key\" != \"null\" ]]" "crawler creation returned no crawler_key"

seed_payload="$(json_request POST "$API_BASE_URL/v1/admin/frontier/seed" "{\"urls\":[\"$seed_url\"],\"source\":\"smoke-test\",\"max_depth\":1,\"allow_revisit\":true}" "$admin_token")"
echo "$seed_payload" | jq -e '.accepted_urls >= 1' >/dev/null
echo "$seed_payload" | jq -e '.frontier_depth >= 1 and .known_urls >= 1' >/dev/null
echo "PASS  frontier seeded"

run_worker() {
  local -a worker_env=(
    env
    NO_PROXY=127.0.0.1,localhost,host.docker.internal
    no_proxy=127.0.0.1,localhost,host.docker.internal
    HTTP_PROXY=
    HTTPS_PROXY=
    ALL_PROXY=
    http_proxy=
    https_proxy=
    all_proxy=
  )
  if command -v findverse-crawler >/dev/null 2>&1; then
    "${worker_env[@]}" findverse-crawler worker --server "$CRAWLER_SERVER" --crawler-id "$crawler_id" --crawler-key "$crawler_key" --once --max-jobs 10
  else
    "${worker_env[@]}" cargo run -p findverse-crawler -- worker --server "$CRAWLER_SERVER" --crawler-id "$crawler_id" --crawler-key "$crawler_key" --once --max-jobs 10
  fi
}

overview_payload=""
documents_payload=""
document_payload=""
search_smoke_payload=""
for _ in 1 2 3 4 5; do
  pushd "$repo_root" >/dev/null
  run_worker
  popd >/dev/null
  overview_payload="$(json_request GET "$API_BASE_URL/v1/admin/crawl/overview" "" "$admin_token")"
  documents_payload="$(json_request GET "$API_BASE_URL/v1/admin/documents?query=findverse-smoke=$timestamp&limit=50" "" "$admin_token")"
  document_payload="$(echo "$documents_payload" | jq -c '.documents[] | select(.source_job_id != null)' | head -n 1)"
  search_smoke_payload="$(json_request GET "$API_BASE_URL/v1/search?q=smoke%20crawler%20fixture")"
  if [[ -n "$document_payload" ]] \
    && echo "$overview_payload" | jq -e '.crawlers | any(.jobs_claimed > 0 and .jobs_reported > 0)' >/dev/null \
    && echo "$search_smoke_payload" | jq -e '.results | any(.url | contains("smoke-crawler.html"))' >/dev/null; then
    break
  fi
  sleep 1
done

echo "$overview_payload" | jq -e '.crawlers | any(.jobs_claimed > 0 and .jobs_reported > 0)' >/dev/null
echo "PASS  crawler claimed and reported jobs"

if [[ -z "$document_payload" || "$document_payload" == "null" ]]; then
  echo "Assertion failed: indexed document for smoke crawl not found" >&2
  exit 1
fi
echo "$document_payload" | jq -e '.canonical_url != null and .host != null and .content_type != null and .word_count >= 1 and has("source_job_id")' >/dev/null
echo "PASS  document metadata fields are present"
echo "$search_smoke_payload" | jq -e '.results | any(.url | contains("smoke-crawler.html"))' >/dev/null
echo "PASS  smoke document is searchable"

step "Docker rebuild verification"
echo "Rebuild the main stack directly with Docker Compose when needed:"
echo "  docker compose up -d --build"

if $RUN_PLAYWRIGHT; then
  step "Playwright"
  export PLAYWRIGHT_BASE_URL="${API_BASE_URL%/api}"
  export PLAYWRIGHT_API_BASE_URL="$API_BASE_URL"
  pushd "$repo_root" >/dev/null
  npx playwright test
  popd >/dev/null
fi

step "Smoke test summary"
echo "PASS  search, suggest, developer auth, key revocation, admin login, crawler credential issue/claim/report, indexed document metadata, smoke search result"
