#!/usr/bin/env bash
set -euo pipefail

BIN_DIR="${HOME}/.local/bin"
mkdir -p "${BIN_DIR}"

OS=$(uname -s)
case "${OS}" in
  Linux)
    NAME="dimclaw-linux-x86_64"
    ;;
  Darwin)
    NAME="dimclaw-macos-x86_64"
    ;;
  *)
    echo "Unsupported OS: ${OS}" >&2
    exit 1
    ;;
esac

URL="https://github.com/zylzyqzz/DimClaw/releases/latest/download/${NAME}"
TARGET="${BIN_DIR}/dimclaw"

curl -fsSL "${URL}" -o "${TARGET}"
chmod +x "${TARGET}"
echo "DimClaw 已安装到 ${TARGET}"
echo "是否立即启动 Web UI? (y/n)"
read -r ans
if [[ "${ans}" == "y" || "${ans}" == "Y" ]]; then
  nohup "${TARGET}" server >/tmp/dimclaw_server.log 2>&1 &
  sleep 1
  if command -v xdg-open >/dev/null 2>&1; then
    xdg-open "http://127.0.0.1:8080" >/dev/null 2>&1 || true
  elif command -v open >/dev/null 2>&1; then
    open "http://127.0.0.1:8080" || true
  fi
  echo "DimClaw Web UI 已启动: http://127.0.0.1:8080"
fi
