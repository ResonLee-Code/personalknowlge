#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
stack="$root/infra/searxng"
env_file="$stack/.env"

if ! command -v docker >/dev/null 2>&1; then
  echo "Docker 未安装，无法启动本机 SearXNG。" >&2
  exit 1
fi
if ! docker info >/dev/null 2>&1; then
  echo "Docker 引擎未运行：请先启动 Docker Desktop 或 Colima。" >&2
  exit 1
fi
if ! command -v openssl >/dev/null 2>&1; then
  echo "缺少 openssl，无法生成本机 SearXNG 会话密钥。" >&2
  exit 1
fi
if docker compose version >/dev/null 2>&1; then
  compose=(docker compose)
elif command -v docker-compose >/dev/null 2>&1; then
  compose=(docker-compose)
else
  echo "缺少 Docker Compose，可用 Homebrew 安装：brew install docker-compose" >&2
  exit 1
fi

upsert_env() {
  local key="$1"
  local value="$2"
  local temp
  temp="$(mktemp "${env_file}.tmp.XXXXXX")"
  awk -v key="$key" -v value="$value" '
    BEGIN { replaced = 0 }
    index($0, key "=") == 1 { print key "=" value; replaced = 1; next }
    { print }
    END { if (!replaced) print key "=" value }
  ' "$env_file" >"$temp"
  mv "$temp" "$env_file"
  chmod 600 "$env_file"
}

if [[ ! -f "$env_file" ]]; then
  umask 077
  secret="$(openssl rand -hex 32)"
  printf 'SEARXNG_PORT=8080\nSEARXNG_SECRET=%s\n' "$secret" >"$env_file"
  echo "已生成本机 SearXNG 私有配置：$env_file"
fi
chmod 600 "$env_file"

# Docker 容器不会继承 macOS 的系统代理。Colima 场景下把宿主机代理地址
# 显式传给容器；没有代理时保持为空，继续走 Docker 的直接出网路径。
if [[ "$(uname -s)" == "Darwin" ]] && command -v scutil >/dev/null 2>&1; then
  proxy_enabled="$(scutil --proxy | awk '$1 == "HTTPEnable" { print $3; exit }')"
  proxy_port="$(scutil --proxy | awk '$1 == "HTTPPort" { print $3; exit }')"
  proxy_url=""
  if [[ "$proxy_enabled" == "1" && -n "$proxy_port" ]]; then
    proxy_host=""
    if command -v colima >/dev/null 2>&1; then
      proxy_host="$(colima ssh -- ip route 2>/dev/null | awk '$1 == "default" { print $3; exit }')"
    fi
    proxy_host="${proxy_host:-host.docker.internal}"
    proxy_url="http://${proxy_host}:${proxy_port}"
  fi
  upsert_env "SEARXNG_PROXY_URL" "$proxy_url"
fi

port="$(awk -F= '$1 == "SEARXNG_PORT" { print $2 }' "$env_file")"
port="${port:-8080}"
base_url="http://127.0.0.1:${port}"

"${compose[@]}" \
  --project-directory "$stack" \
  --env-file "$env_file" \
  -f "$stack/compose.yml" \
  up -d

ready=0
for _ in $(seq 1 60); do
  if curl --fail --silent --show-error --max-time 2 "$base_url/healthz" >/dev/null 2>&1; then
    ready=1
    break
  fi
  sleep 1
done
if [[ "$ready" != 1 ]]; then
  echo "SearXNG 未能在 60 秒内就绪，最近日志：" >&2
  "${compose[@]}" \
    --project-directory "$stack" \
    --env-file "$env_file" \
    -f "$stack/compose.yml" \
    logs --tail 80 >&2
  exit 1
fi

body="$(curl --fail --silent --show-error --max-time 20 \
  --get "$base_url/search" \
  --data-urlencode "q=SearXNG 官方文档" \
  --data "format=json" \
  --data "language=zh-CN")"
count="$(printf '%s' "$body" | node -e '
let raw = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", (chunk) => raw += chunk);
process.stdin.on("end", () => {
  const value = JSON.parse(raw);
  process.stdout.write(String(Array.isArray(value.results) ? value.results.length : 0));
});
')"
if [[ "$count" == 0 ]]; then
  echo "SearXNG 已启动，但连接测试没有返回结果；请查看容器日志或稍后重试。" >&2
  exit 1
fi

echo "SearXNG 已就绪，真实 JSON 检索返回 ${count} 条候选。"
echo "应用检索源地址："
echo "${base_url}/search?q={query}&format=json&count={limit}&language=zh-CN"

