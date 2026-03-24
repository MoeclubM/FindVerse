#!/usr/bin/env bash
set -euo pipefail

# FindVerse Crawler Setup
# Usage: ./crawler-setup.sh --server https://api.example.com --join-key <key> [--name <name>] [--start] [--concurrency <N>]
#
# This script:
# 1. Calls POST /internal/crawlers/join to register a crawler and get credentials
# 2. Writes a .env file with CRAWLER_ID, CRAWLER_KEY, SERVER
# 3. Optionally starts the crawler worker

# Features:
# - Works with just curl + jq (no Rust/cargo needed if binary is pre-built)
# - If cargo is available and no binary found, builds from source
# - Caches credentials in .env so restarts don't re-register

SERVER=""
JOIN_KEY=""
NAME="worker-$(hostname 2>/dev/null || echo unknown)"
START=false
CONCURRENCY=4
MAX_JOBS=10
POLL_INTERVAL_SECS=5
ALLOWED_DOMAINS=""
PROXY=""
BIN_PATH=""
ENV_FILE=".env.crawler"

while [[ $# -gt 0 ]]; do
  case $1 in
    --server) SERVER="$2"; shift 2 ;;
    --join-key) JOIN_KEY="$2"; shift 2 ;;
    --name) NAME="$2"; shift 2 ;;
    --start) START=true; shift ;;
    --concurrency) CONCURRENCY="$2"; shift 2 ;;
    --max-jobs) MAX_JOBS="$2"; shift 2 ;;
    --poll-interval-secs) POLL_INTERVAL_SECS="$2"; shift 2 ;;
    --allowed-domains) ALLOWED_DOMAINS="$2"; shift 2 ;;
    --proxy) PROXY="$2"; shift 2 ;;
    --bin-path) BIN_PATH="$2"; shift 2 ;;
    --env-file) ENV_FILE="$2"; shift 2 ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

if [[ -z "$SERVER" || -z "$JOIN_KEY" ]]; then
  echo "Usage: $0 --server <url> --join-key <key> [--name <name>] [--start] [--concurrency <N>]"
  exit 1
fi

start_worker() {
  local -a args=(
    worker
    --server "$SERVER"
    --crawler-id "$CRAWLER_ID"
    --crawler-key "$CRAWLER_KEY"
    --max-jobs "$MAX_JOBS"
    --poll-interval-secs "$POLL_INTERVAL_SECS"
    --concurrency "$CONCURRENCY"
  )

  if [[ -n "$ALLOWED_DOMAINS" ]]; then
    args+=(--allowed-domains "$ALLOWED_DOMAINS")
  fi
  if [[ -n "$PROXY" ]]; then
    args+=(--proxy "$PROXY")
  fi

  if [[ -n "$BIN_PATH" ]]; then
    exec "$BIN_PATH" "${args[@]}"
  elif command -v findverse-crawler &>/dev/null; then
    exec findverse-crawler "${args[@]}"
  elif command -v cargo &>/dev/null; then
    exec cargo run -p findverse-crawler -- "${args[@]}"
  else
    echo "Error: neither findverse-crawler binary nor cargo found"
    exit 1
  fi
}

# Check if already registered
if [[ -f "$ENV_FILE" ]]; then
  echo "Found existing credentials in $ENV_FILE"
  source "$ENV_FILE"
  if [[ -n "${CRAWLER_ID:-}" && -n "${CRAWLER_KEY:-}" ]]; then
    echo "  Crawler ID: $CRAWLER_ID"
    echo "  Using cached credentials. Delete $ENV_FILE to re-register."
    if $START; then
      echo "Starting crawler worker..."
      start_worker
    fi
    exit 0
  fi
fi

# Register via join endpoint
echo "Registering crawler '$NAME' with server $SERVER..."

RESPONSE=$(curl -sf -X POST "${SERVER%/}/internal/crawlers/join" \
  -H "Content-Type: application/json" \
  -d "{\"join_key\": \"$JOIN_KEY\", \"name\": \"$NAME\"}")

if [[ $? -ne 0 || -z "$RESPONSE" ]]; then
  echo "Error: Failed to register. Check --server and --join-key values."
  exit 1
fi

CRAWLER_ID=$(echo "$RESPONSE" | jq -r '.crawler_id')
CRAWLER_KEY=$(echo "$RESPONSE" | jq -r '.crawler_key')
RETURNED_NAME=$(echo "$RESPONSE" | jq -r '.name')

if [[ -z "$CRAWLER_ID" || "$CRAWLER_ID" == "null" ]]; then
  echo "Error: Invalid response from server"
  echo "$RESPONSE"
  exit 1
fi

echo "Registered successfully!"
echo "  Crawler ID:   $CRAWLER_ID"
echo "  Crawler name: $RETURNED_NAME"

# Save credentials
cat > "$ENV_FILE" <<ENVEOF
CRAWLER_ID=$CRAWLER_ID
CRAWLER_KEY=$CRAWLER_KEY
SERVER=$SERVER
ENVEOF

echo "Credentials saved to $ENV_FILE"

if $START; then
  echo "Starting crawler worker..."
  start_worker
else
  echo ""
  echo "To start the crawler manually:"
  echo "  findverse-crawler worker --server $SERVER --crawler-id $CRAWLER_ID --crawler-key \$CRAWLER_KEY --max-jobs $MAX_JOBS --poll-interval-secs $POLL_INTERVAL_SECS --concurrency $CONCURRENCY"
fi
