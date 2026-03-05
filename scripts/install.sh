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
