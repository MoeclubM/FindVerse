#!/usr/bin/env bash
set -euo pipefail

ENV_FILE=".env.production"
COMPOSE_FILE="docker-compose.yml"
PULL_IMAGES=true
PASSTHROUGH=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --env-file)
      ENV_FILE="$2"
      shift 2
      ;;
    --compose-file)
      COMPOSE_FILE="$2"
      shift 2
      ;;
    --skip-pull)
      PULL_IMAGES=false
      shift
      ;;
    --with-crawler)
      echo "deploy-prod.sh does not support the Docker crawler profile; deploy crawler nodes with scripts/crawler-setup.sh instead." >&2
      exit 1
      ;;
    *)
      PASSTHROUGH+=("$1")
      shift
      ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$REPO_ROOT"

[[ -f "$ENV_FILE" ]] || {
  echo "deploy-prod.sh could not find env file: ${ENV_FILE}" >&2
  exit 1
}

require_env() {
  local name="$1"
  local value
  value="$(grep -E "^${name}=" "$ENV_FILE" | tail -n 1 | cut -d= -f2- || true)"
  [[ -n "$value" ]] || {
    echo "deploy-prod.sh requires ${name} in ${ENV_FILE}" >&2
    exit 1
  }
}

require_env "FINDVERSE_IMAGE_PREFIX"
require_env "FINDVERSE_IMAGE_TAG"
require_env "FINDVERSE_FRONTEND_ORIGIN"
require_env "FINDVERSE_LOCAL_ADMIN_PASSWORD"
require_env "FINDVERSE_POSTGRES_PASSWORD"
require_env "FINDVERSE_CRAWLER_JOIN_KEY"

compose_args=(--env-file "$ENV_FILE" -f "$COMPOSE_FILE")

if $PULL_IMAGES; then
  docker compose "${compose_args[@]}" pull
fi

exec "$SCRIPT_DIR/deploy-stack.sh" \
  --env-file "$ENV_FILE" \
  --compose-file "$COMPOSE_FILE" \
  "${PASSTHROUGH[@]}"
