#!/usr/bin/env bash
set -euo pipefail

REPO="${FINDVERSE_GITHUB_REPO:-MoeclubM/FindVerse}"
CHANNEL="release"
VERSION=""
SERVER_URL=""
CRAWLER_KEY_ARG=""
SERVICE_NAME="findverse-crawler"
INSTALL_DIR="/opt/findverse-crawler"
ENV_FILE="/etc/findverse-crawler/crawler.env"
MAX_JOBS=""
POLL_INTERVAL_SECS=""
ALLOWED_DOMAINS=""
PROXY=""
GITHUB_TOKEN="${GITHUB_TOKEN:-}"
SKIP_BROWSER_INSTALL=false

TMP_DIR=""
SOURCE_LABEL=""
DOWNLOADED_ARCHIVE_PATH=""

usage() {
  cat <<'EOF'
Usage:
  install-crawler.sh --server <url> [options]

Install or update the FindVerse crawler binary and systemd service on this machine.
The script downloads either the latest GitHub release artifact or the latest CI dev artifact,
installs the binary into /opt, writes config into /etc, and enables a systemd service.

Options:
  --server <url>                Control API base URL. Required on first install, reused later if omitted
  --crawler-key <key>           Shared fixed crawler key. Required on first install, reused later if omitted
  --channel <release|dev>       Download source. Default: release
  --version <tag>               Optional pinned release tag, for example v1.2.3
  --repo <owner/name>           GitHub repo. Default: MoeclubM/FindVerse
  --service-name <name>         systemd service name. Default: findverse-crawler
  --install-dir <dir>           Install directory. Default: /opt/findverse-crawler
  --env-file <path>             Config file path. Default: /etc/findverse-crawler/crawler.env
  --max-jobs <n>                Claim batch size. Defaults to concurrency when omitted
  --poll-interval-secs <n>      Poll interval. Reuses existing config if omitted
  --allowed-domains <csv>       Optional domain allowlist
  --proxy <url>                 Optional outbound proxy
  --github-token <token>        GitHub token. Only needed for --channel dev
  --skip-browser-install        Do not auto-install Chromium when missing
  --help                        Show this help

Notes:
  - This script is standalone and can be downloaded or piped directly from GitHub onto the target machine.
  - Re-running the script updates the binary in place and restarts the service.
  - On first install the script auto-generates crawler_id and saves it into the env file.
  - On first install the script uses the local hostname as the default crawler name.
  - Once the env file exists, updates can reuse the saved server, crawler_id, and crawler_key.
  - The same release command can be used for both first install and updates.
  - Release mode downloads the public GitHub release asset without auth.
  - Dev mode downloads the latest successful crawler dev build artifact and requires GitHub API auth even for public repos.
EOF
}

fail() {
  echo "Error: $*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

cleanup() {
  if [[ -n "$TMP_DIR" && -d "$TMP_DIR" ]]; then
    rm -rf "$TMP_DIR"
  fi
}

trap cleanup EXIT

while [[ $# -gt 0 ]]; do
  case "$1" in
    --server) SERVER_URL="$2"; shift 2 ;;
    --crawler-key) CRAWLER_KEY_ARG="$2"; shift 2 ;;
    --channel) CHANNEL="$2"; shift 2 ;;
    --version) VERSION="$2"; shift 2 ;;
    --repo) REPO="$2"; shift 2 ;;
    --service-name) SERVICE_NAME="$2"; shift 2 ;;
    --install-dir) INSTALL_DIR="$2"; shift 2 ;;
    --env-file) ENV_FILE="$2"; shift 2 ;;
    --max-jobs) MAX_JOBS="$2"; shift 2 ;;
    --poll-interval-secs) POLL_INTERVAL_SECS="$2"; shift 2 ;;
    --allowed-domains) ALLOWED_DOMAINS="$2"; shift 2 ;;
    --proxy) PROXY="$2"; shift 2 ;;
    --github-token) GITHUB_TOKEN="$2"; shift 2 ;;
    --skip-browser-install) SKIP_BROWSER_INSTALL=true; shift ;;
    --help|-h) usage; exit 0 ;;
    *) fail "unknown option: $1" ;;
  esac
done

[[ "$CHANNEL" == "release" || "$CHANNEL" == "dev" ]] || fail "--channel must be release or dev"

require_cmd curl
require_cmd jq
require_cmd tar
require_cmd unzip
require_cmd systemctl
require_cmd install
require_cmd mktemp

AS_ROOT=()
if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
  command -v sudo >/dev/null 2>&1 || fail "this script needs root or sudo"
  AS_ROOT=(sudo)
fi

run_as_root() {
  "${AS_ROOT[@]}" "$@"
}

generate_crawler_id() {
  if [[ -r /proc/sys/kernel/random/uuid ]]; then
    tr '[:upper:]' '[:lower:]' </proc/sys/kernel/random/uuid
    return
  fi

  if command -v uuidgen >/dev/null 2>&1; then
    uuidgen | tr '[:upper:]' '[:lower:]'
    return
  fi

  fail "unable to generate crawler id automatically"
}

machine_suffix() {
  case "$(uname -m)" in
    x86_64|amd64) echo "linux-x86_64" ;;
    *) fail "unsupported architecture: $(uname -m)" ;;
  esac
}

github_api_json() {
  local url="$1"
  local require_auth="${2:-false}"
  local -a args=(
    -fsSL
    -H "Accept: application/vnd.github+json"
    -H "X-GitHub-Api-Version: 2022-11-28"
  )
  if [[ -n "$GITHUB_TOKEN" ]]; then
    args+=(-H "Authorization: Bearer $GITHUB_TOKEN")
  elif [[ "$require_auth" == "true" ]]; then
    fail "GitHub token is required for this operation"
  fi
  curl "${args[@]}" "$url"
}

github_download() {
  local url="$1"
  local output="$2"
  local require_auth="${3:-false}"
  local -a args=(-fsSL -o "$output")
  if [[ -n "$GITHUB_TOKEN" ]]; then
    args+=(-H "Authorization: Bearer $GITHUB_TOKEN")
  elif [[ "$require_auth" == "true" ]]; then
    fail "GitHub token is required for this download"
  fi
  curl "${args[@]}" "$url"
}

browser_exists() {
  command -v chromium >/dev/null 2>&1 \
    || command -v chromium-browser >/dev/null 2>&1 \
    || command -v google-chrome >/dev/null 2>&1 \
    || command -v google-chrome-stable >/dev/null 2>&1
}

ensure_browser() {
  $SKIP_BROWSER_INSTALL && return
  browser_exists && return

  if command -v apt-get >/dev/null 2>&1; then
    run_as_root env DEBIAN_FRONTEND=noninteractive apt-get update
    run_as_root env DEBIAN_FRONTEND=noninteractive apt-get install -y chromium
    return
  fi

  if command -v dnf >/dev/null 2>&1; then
    run_as_root dnf install -y chromium
    return
  fi

  echo "Warning: Chromium is not installed and no supported package manager was found." >&2
  echo "Warning: JS-rendered pages will fall back to static fetch until Chromium is installed." >&2
}

download_release_archive() {
  local suffix="$1"
  local release_url
  local release_json
  local asset_url
  local archive_path="$TMP_DIR/findverse-${suffix}.tar.gz"

  if [[ -n "$VERSION" ]]; then
    release_url="https://api.github.com/repos/${REPO}/releases/tags/${VERSION}"
    SOURCE_LABEL="release:${VERSION}"
  else
    release_url="https://api.github.com/repos/${REPO}/releases/latest"
    SOURCE_LABEL="release:latest"
  fi

  release_json="$(github_api_json "$release_url")"
  asset_url="$(printf '%s' "$release_json" | jq -r --arg name "findverse-${suffix}.tar.gz" '.assets[] | select(.name == $name) | .browser_download_url' | head -n 1)"
  [[ -n "$asset_url" && "$asset_url" != "null" ]] || fail "release asset findverse-${suffix}.tar.gz not found for ${REPO}"

  github_download "$asset_url" "$archive_path"
  DOWNLOADED_ARCHIVE_PATH="$archive_path"
}

download_dev_archive() {
  local suffix="$1"
  local runs_json
  local run_id
  local artifacts_json
  local artifact_url
  local zip_path="$TMP_DIR/crawler-dev-artifact.zip"
  local artifact_dir="$TMP_DIR/dev-artifact"
  local archive_path

  runs_json="$(github_api_json "https://api.github.com/repos/${REPO}/actions/workflows/crawler-dev-artifact.yml/runs?status=success&per_page=20" true)"
  run_id="$(printf '%s' "$runs_json" | jq -r '.workflow_runs[] | select(.head_branch == "main" or .head_branch == "master") | .id' | head -n 1)"
  [[ -n "$run_id" ]] || fail "no successful CI runs found on main/master for ${REPO}"

  artifacts_json="$(github_api_json "https://api.github.com/repos/${REPO}/actions/runs/${run_id}/artifacts?per_page=100" true)"
  artifact_url="$(printf '%s' "$artifacts_json" | jq -r --arg name "findverse-crawler-${suffix}-dev" '.artifacts[] | select(.name == $name and .expired == false) | .archive_download_url' | head -n 1)"
  [[ -n "$artifact_url" ]] || fail "dev artifact findverse-crawler-${suffix}-dev not found on CI run ${run_id}"

  SOURCE_LABEL="dev:run-${run_id}"
  github_download "$artifact_url" "$zip_path" true
  mkdir -p "$artifact_dir"
  unzip -q "$zip_path" -d "$artifact_dir"

  archive_path="$(find "$artifact_dir" -maxdepth 2 -type f -name "findverse-crawler-${suffix}.tar.gz" | head -n 1)"
  [[ -n "$archive_path" ]] || fail "downloaded dev artifact did not contain findverse-crawler-${suffix}.tar.gz"

  DOWNLOADED_ARCHIVE_PATH="$archive_path"
}

extract_crawler_binary() {
  local archive_path="$1"
  local unpack_dir="$TMP_DIR/unpack"
  local binary_path

  mkdir -p "$unpack_dir"
  tar -xzf "$archive_path" -C "$unpack_dir"
  binary_path="$(find "$unpack_dir" -maxdepth 2 -type f -name "findverse-crawler" | head -n 1)"
  [[ -n "$binary_path" ]] || fail "archive did not contain findverse-crawler"

  echo "$binary_path"
}

load_existing_config() {
  EXISTING_SERVER=""
  EXISTING_CRAWLER_ID=""
  EXISTING_CRAWLER_NAME=""
  EXISTING_CRAWLER_KEY=""
  EXISTING_MAX_JOBS=""
  EXISTING_POLL_INTERVAL_SECS=""
  EXISTING_ALLOWED_DOMAINS=""
  EXISTING_PROXY=""

  if [[ -f "$ENV_FILE" ]]; then
    # shellcheck disable=SC1090
    source "$ENV_FILE"
    EXISTING_SERVER="${SERVER:-}"
    EXISTING_CRAWLER_ID="${CRAWLER_ID:-}"
    EXISTING_CRAWLER_NAME="${CRAWLER_NAME:-}"
    EXISTING_CRAWLER_KEY="${CRAWLER_KEY:-}"
    EXISTING_MAX_JOBS="${MAX_JOBS:-}"
    EXISTING_POLL_INTERVAL_SECS="${POLL_INTERVAL_SECS:-}"
    EXISTING_ALLOWED_DOMAINS="${ALLOWED_DOMAINS:-}"
    EXISTING_PROXY="${PROXY:-}"
  fi
}

default_crawler_name() {
  local detected=""

  if command -v hostname >/dev/null 2>&1; then
    detected="$(hostname 2>/dev/null || true)"
  fi

  if [[ -z "$detected" && -r /etc/hostname ]]; then
    detected="$(cat /etc/hostname 2>/dev/null || true)"
  fi

  detected="${detected//$'\r'/}"
  detected="${detected//$'\n'/}"
  detected="${detected#"${detected%%[![:space:]]*}"}"
  detected="${detected%"${detected##*[![:space:]]}"}"

  if [[ -n "$detected" ]]; then
    printf '%s' "$detected"
    return
  fi

  printf 'crawler'
}

write_env_file() {
  local final_crawler_id="$1"
  local final_crawler_name="$2"
  local final_crawler_key="$3"
  local env_dir
  local env_tmp="$TMP_DIR/crawler.env"
  local final_max_jobs final_poll_interval
  local final_allowed_domains final_proxy

  final_max_jobs="${MAX_JOBS:-${EXISTING_MAX_JOBS:-16}}"
  final_poll_interval="${POLL_INTERVAL_SECS:-${EXISTING_POLL_INTERVAL_SECS:-5}}"
  final_allowed_domains="${ALLOWED_DOMAINS:-${EXISTING_ALLOWED_DOMAINS:-}}"
  final_proxy="${PROXY:-${EXISTING_PROXY:-}}"

  env_dir="$(dirname "$ENV_FILE")"
  run_as_root mkdir -p "$env_dir"

  cat > "$env_tmp" <<EOF
SERVER=$SERVER_URL
CRAWLER_ID=$final_crawler_id
CRAWLER_NAME=$final_crawler_name
CRAWLER_KEY=$final_crawler_key
MAX_JOBS=$final_max_jobs
POLL_INTERVAL_SECS=$final_poll_interval
ALLOWED_DOMAINS=$final_allowed_domains
PROXY=$final_proxy
EOF

  run_as_root install -m 600 "$env_tmp" "$ENV_FILE"
}

install_runtime_files() {
  local binary_source="$1"
  local launcher_tmp="$TMP_DIR/run-crawler.sh"

  cat > "$launcher_tmp" <<EOF
#!/usr/bin/env bash
set -euo pipefail

args=(
  worker
  --server "\${SERVER}"
  --crawler-id "\${CRAWLER_ID}"
  --crawler-name "\${CRAWLER_NAME:-}"
  --crawler-key "\${CRAWLER_KEY}"
  --max-jobs "\${MAX_JOBS:-16}"
  --poll-interval-secs "\${POLL_INTERVAL_SECS:-5}"
)
if [[ -n "\${ALLOWED_DOMAINS:-}" ]]; then
  args+=(--allowed-domains "\${ALLOWED_DOMAINS}")
fi
if [[ -n "\${PROXY:-}" ]]; then
  args+=(--proxy "\${PROXY}")
fi
exec "${INSTALL_DIR}/findverse-crawler" "\${args[@]}"
EOF

  run_as_root mkdir -p "$INSTALL_DIR"
  run_as_root install -m 755 "$binary_source" "$INSTALL_DIR/findverse-crawler.new"
  run_as_root mv "$INSTALL_DIR/findverse-crawler.new" "$INSTALL_DIR/findverse-crawler"
  run_as_root install -m 755 "$launcher_tmp" "$INSTALL_DIR/run-crawler.sh"
}

write_service_unit() {
  local service_path="/etc/systemd/system/${SERVICE_NAME}.service"

  run_as_root tee "$service_path" >/dev/null <<EOF
[Unit]
Description=FindVerse crawler worker (${SERVICE_NAME})
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
EnvironmentFile=${ENV_FILE}
ExecStart=${INSTALL_DIR}/run-crawler.sh
Restart=always
RestartSec=5
TimeoutStopSec=600
WorkingDirectory=${INSTALL_DIR}

[Install]
WantedBy=multi-user.target
EOF
}

main() {
  local suffix archive_path binary_path final_crawler_id final_crawler_name final_crawler_key

  TMP_DIR="$(mktemp -d)"
  suffix="$(machine_suffix)"
  load_existing_config
  SERVER_URL="${SERVER_URL:-${EXISTING_SERVER:-}}"
  [[ -n "$SERVER_URL" ]] || fail "--server is required on first install"
  ensure_browser

  case "$CHANNEL" in
    release)
      if [[ -n "$VERSION" ]]; then
        SOURCE_LABEL="release:${VERSION}"
      else
        SOURCE_LABEL="release:latest"
      fi
      download_release_archive "$suffix"
      archive_path="$DOWNLOADED_ARCHIVE_PATH"
      ;;
    dev)
      download_dev_archive "$suffix"
      archive_path="$DOWNLOADED_ARCHIVE_PATH"
      ;;
  esac

  binary_path="$(extract_crawler_binary "$archive_path")"

  final_crawler_id="${EXISTING_CRAWLER_ID:-}"
  final_crawler_name="${EXISTING_CRAWLER_NAME:-}"
  final_crawler_key="${CRAWLER_KEY_ARG:-${EXISTING_CRAWLER_KEY:-}}"
  if [[ -z "$final_crawler_id" ]]; then
    final_crawler_id="$(generate_crawler_id)"
  fi
  if [[ -z "$final_crawler_name" ]]; then
    final_crawler_name="$(default_crawler_name)"
  fi
  [[ -n "$final_crawler_key" ]] || fail "--crawler-key is required on first install"

  install_runtime_files "$binary_path"
  write_env_file "$final_crawler_id" "$final_crawler_name" "$final_crawler_key"
  write_service_unit

  run_as_root systemctl daemon-reload
  run_as_root systemctl enable --now "${SERVICE_NAME}.service"
  run_as_root systemctl restart "${SERVICE_NAME}.service"

  echo "Installed ${SERVICE_NAME} from ${SOURCE_LABEL}"
  echo "  Repo:        ${REPO}"
  echo "  Install dir: ${INSTALL_DIR}"
  echo "  Env file:    ${ENV_FILE}"
  echo "  Server:      ${SERVER_URL}"
  echo "  Crawler ID:  ${final_crawler_id}"
  echo "  Crawler Name:${final_crawler_name}"
  echo "  Channel:     ${CHANNEL}"
}

main "$@"
