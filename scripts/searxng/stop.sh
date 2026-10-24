#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
stack="$root/infra/searxng"
env_file="$stack/.env"

if [[ ! -f "$env_file" ]]; then
  echo "本机 SearXNG 尚未初始化。"
  exit 0
fi
if docker compose version >/dev/null 2>&1; then
  compose=(docker compose)
elif command -v docker-compose >/dev/null 2>&1; then
  compose=(docker-compose)
else
  echo "缺少 Docker Compose，可用 Homebrew 安装：brew install docker-compose" >&2
  exit 1
fi

"${compose[@]}" \
  --project-directory "$stack" \
  --env-file "$env_file" \
  -f "$stack/compose.yml" \
  down

