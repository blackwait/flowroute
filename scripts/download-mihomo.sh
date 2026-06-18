#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN_DIR="$ROOT/src-tauri/binaries"
VERSION="v1.19.12"
ARCH="$(uname -m)"

case "$ARCH" in
  x86_64) TARGET="mihomo-darwin-amd64-${VERSION}.gz" ;;
  arm64)  TARGET="mihomo-darwin-arm64-${VERSION}.gz" ;;
  *) echo "Unsupported arch: $ARCH"; exit 1 ;;
esac

URL="https://github.com/MetaCubeX/mihomo/releases/download/${VERSION}/${TARGET}"
OUT="$BIN_DIR/mihomo"

mkdir -p "$BIN_DIR"
echo "Downloading mihomo ${VERSION} ..."
for TRY_URL in \
  "https://gh-proxy.com/${URL}" \
  "https://mirror.ghproxy.com/${URL}" \
  "${URL}"; do
  echo "Trying $TRY_URL"
  if curl -L --connect-timeout 20 --max-time 180 "$TRY_URL" -o "/tmp/${TARGET}" && [ -s "/tmp/${TARGET}" ]; then
    gunzip -c "/tmp/${TARGET}" > "$OUT"
    chmod +x "$OUT"
    ln -sf mihomo "$BIN_DIR/mihomo-x86_64-apple-darwin"
    ln -sf mihomo "$BIN_DIR/mihomo-aarch64-apple-darwin"
    rm -f "/tmp/${TARGET}"
    echo "Installed: $OUT"
    "$OUT" -v || true
    exit 0
  fi
done

echo "下载失败，请检查网络后重试"
exit 1
