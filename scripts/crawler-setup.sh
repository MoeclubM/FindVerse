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
# - JS rendering is enabled by default in current crawler builds
# - Caches credentials in .env so restarts don't re-register

SERVER=""
JOIN_KEY=""
NAME="worker-$(hostname 2>/dev/null || echo unknown)"
START=false
INSTALL_SERVICE=false
CONCURRENCY=4
MAX_JOBS=10
POLL_INTERVAL_SECS=5
ALLOWED_DOMAINS=""
PROXY=""
LLM_BASE_URL=""
LLM_API_KEY=""
LLM_MODEL=""
LLM_MIN_SCORE="0.45"
LLM_MAX_BODY_CHARS="6000"
BIN_PATH=""
ENV_FILE=".env.crawler"
SERVICE_NAME="findverse-crawler"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SCRIPT_PATH="$SCRIPT_DIR/$(basename "${BASH_SOURCE[0]}")"

while [[ $# -gt 0 ]]; do
  case $1 in
    --server) SERVER="$2"; shift 2 ;;
    --join-key) JOIN_KEY="$2"; shift 2 ;;
    --name) NAME="$2"; shift 2 ;;
    --start) START=true; shift ;;
    --install-service) INSTALL_SERVICE=true; shift ;;
    --concurrency) CONCURRENCY="$2"; shift 2 ;;
    --max-jobs) MAX_JOBS="$2"; shift 2 ;;
    --poll-interval-secs) POLL_INTERVAL_SECS="$2"; shift 2 ;;
    --allowed-domains) ALLOWED_DOMAINS="$2"; shift 2 ;;
    --proxy) PROXY="$2"; shift 2 ;;
    --llm-base-url) LLM_BASE_URL="$2"; shift 2 ;;
    --llm-api-key) LLM_API_KEY="$2"; shift 2 ;;
    --llm-model) LLM_MODEL="$2"; shift 2 ;;
    --llm-min-score) LLM_MIN_SCORE="$2"; shift 2 ;;
    --llm-max-body-chars) LLM_MAX_BODY_CHARS="$2"; shift 2 ;;
    --bin-path) BIN_PATH="$2"; shift 2 ;;
    --env-file) ENV_FILE="$2"; shift 2 ;;
    --service-name) SERVICE_NAME="$2"; shift 2 ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

if [[ -z "$SERVER" || -z "$JOIN_KEY" ]]; then
  echo "Usage: $0 --server <url> --join-key <key> [--name <name>] [--start] [--install-service] [--service-name <name>] [--concurrency <N>]"
  exit 1
fi

warn_if_browser_missing() {
  if command -v chromium >/dev/null 2>&1 \
    || command -v chromium-browser >/dev/null 2>&1 \
    || command -v google-chrome >/dev/null 2>&1 \
    || command -v google-chrome-stable >/dev/null 2>&1 \
    || command -v microsoft-edge >/dev/null 2>&1; then
    return
  fi

  echo "Warning: no local Chromium/Chrome executable found." >&2
  echo "Warning: JS rendering is enabled by default, but pages will fall back to static HTML until a browser is installed." >&2
}

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
  if [[ -n "$LLM_BASE_URL" && -n "$LLM_MODEL" ]]; then
    args+=(--llm-base-url "$LLM_BASE_URL" --llm-model "$LLM_MODEL" --llm-min-score "$LLM_MIN_SCORE" --llm-max-body-chars "$LLM_MAX_BODY_CHARS")
  fi
  if [[ -n "$LLM_API_KEY" ]]; then
    args+=(--llm-api-key "$LLM_API_KEY")
  fi

  if [[ -n "$BIN_PATH" ]]; then
    exec "$BIN_PATH" "${args[@]}"
  elif [[ -x "$REPO_ROOT/target/debug/findverse-crawler" ]]; then
    exec "$REPO_ROOT/target/debug/findverse-crawler" "${args[@]}"
  elif command -v findverse-crawler &>/dev/null; then
    exec findverse-crawler "${args[@]}"
  elif command -v cargo &>/dev/null; then
    exec cargo run -p findverse-crawler -- "${args[@]}"
  else
    echo "Error: neither findverse-crawler binary nor cargo found"
    exit 1
  fi
}

install_systemd_service() {
  if ! command -v systemctl &>/dev/null; then
    echo "Error: systemctl not found; cannot install systemd service"
    exit 1
  fi

  local -a service_cmd=(
    "$SCRIPT_PATH"
    --server "$SERVER"
    --join-key "$JOIN_KEY"
    --env-file "$ENV_FILE"
    --start
    --concurrency "$CONCURRENCY"
    --max-jobs "$MAX_JOBS"
    --poll-interval-secs "$POLL_INTERVAL_SECS"
  )

  if [[ -n "$NAME" ]]; then
    service_cmd+=(--name "$NAME")
  fi
  if [[ -n "$ALLOWED_DOMAINS" ]]; then
    service_cmd+=(--allowed-domains "$ALLOWED_DOMAINS")
  fi
  if [[ -n "$PROXY" ]]; then
    service_cmd+=(--proxy "$PROXY")
  fi
  if [[ -n "$LLM_BASE_URL" ]]; then
    service_cmd+=(--llm-base-url "$LLM_BASE_URL")
  fi
  if [[ -n "$LLM_API_KEY" ]]; then
    service_cmd+=(--llm-api-key "$LLM_API_KEY")
  fi
  if [[ -n "$LLM_MODEL" ]]; then
    service_cmd+=(--llm-model "$LLM_MODEL")
  fi
  if [[ -n "$BIN_PATH" ]]; then
    service_cmd+=(--bin-path "$BIN_PATH")
  fi
  service_cmd+=(--llm-min-score "$LLM_MIN_SCORE" --llm-max-body-chars "$LLM_MAX_BODY_CHARS")

  local exec_start
  printf -v exec_start '%q ' /usr/bin/env bash "${service_cmd[@]}"
  exec_start="${exec_start% }"

  local service_path="/etc/systemd/system/${SERVICE_NAME}.service"
  if [[ "${EUID:-$(id -u)}" -ne 0 ]] && ! command -v sudo &>/dev/null; then
    echo "Error: installing a systemd service requires root or sudo"
    exit 1
  fi

  local -a as_root=()
  if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    as_root=(sudo)
  fi

  "${as_root[@]}" tee "$service_path" >/dev/null <<EOF
[Unit]
Description=FindVerse crawler worker (${SERVICE_NAME})
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$REPO_ROOT
ExecStart=$exec_start
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

  "${as_root[@]}" systemctl daemon-reload
  "${as_root[@]}" systemctl enable --now "${SERVICE_NAME}.service"
  "${as_root[@]}" systemctl --no-pager --full status "${SERVICE_NAME}.service" || true
}

# Check if already registered
if [[ -f "$ENV_FILE" ]]; then
  echo "Found existing credentials in $ENV_FILE"
  source "$ENV_FILE"
  if [[ -n "${CRAWLER_ID:-}" && -n "${CRAWLER_KEY:-}" ]]; then
    echo "  Crawler ID: $CRAWLER_ID"
    echo "  Using cached credentials. Delete $ENV_FILE to re-register."
    if $START; then
      warn_if_browser_missing
      echo "Starting crawler worker..."
      start_worker
    elif $INSTALL_SERVICE; then
      warn_if_browser_missing
      echo "Installing systemd service ${SERVICE_NAME}..."
      install_systemd_service
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
LLM_BASE_URL=$LLM_BASE_URL
LLM_MODEL=$LLM_MODEL
ENVEOF

echo "Credentials saved to $ENV_FILE"

if $START; then
  warn_if_browser_missing
  echo "Starting crawler worker..."
  start_worker
elif $INSTALL_SERVICE; then
  warn_if_browser_missing
  echo "Installing systemd service ${SERVICE_NAME}..."
  install_systemd_service
else
  echo ""
  echo "To start the crawler manually:"
  echo "  findverse-crawler worker --server $SERVER --crawler-id $CRAWLER_ID --crawler-key \$CRAWLER_KEY --max-jobs $MAX_JOBS --poll-interval-secs $POLL_INTERVAL_SECS --concurrency $CONCURRENCY"
fi
