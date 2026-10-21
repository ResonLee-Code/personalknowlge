#!/usr/bin/env bash
# 用一次性测试密钥验证 Tauri updater 的 minisign 正负路径。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TMP="$(mktemp -d "${TMPDIR:-/tmp}/pkos-updater-signature.XXXXXX")"
trap 'rm -rf "${TMP}"' EXIT

KEY="${TMP}/updater-test.key"
PASSWORD="pkos-test-only"
PAYLOAD="${TMP}/payload.zip"

cd "${ROOT}"
pnpm tauri signer generate --ci --password "${PASSWORD}" --write-keys "${KEY}" >/dev/null
printf 'Personal Knowledge OS updater signature fixture\n' > "${PAYLOAD}"
TAURI_SIGNING_PRIVATE_KEY_PATH="${KEY}" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="${PASSWORD}" \
pnpm tauri signer sign --app-version 2.0.0 "${PAYLOAD}" >/dev/null

PKOS_TEST_UPDATER_PUBKEY="${KEY}.pub" \
PKOS_TEST_UPDATER_SIGNATURE="${PAYLOAD}.sig" \
PKOS_TEST_UPDATER_PAYLOAD="${PAYLOAD}" \
cargo test --offline --manifest-path src-tauri/Cargo.toml \
  --test updater_signature_flow -- --nocapture

echo "PKOS_UPDATER_SIGNATURE_VERIFY valid=passed tampered=rejected version=2.0.0"
