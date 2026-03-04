#!/usr/bin/env bash
set -euo pipefail

BIN_DIR="${HOME}/.local/bin"
mkdir -p "${BIN_DIR}"

LATEST=$(curl -fsSL https://api.github.com/repos/zylzyqzz/DimClaw/releases/latest | jq -r '.tag_name')
URL="https://github.com/zylzyqzz/DimClaw/releases/latest/download/dimclaw-linux-x86_64"

curl -fsSL "${URL}" -o "${BIN_DIR}/dimclaw"
chmod +x "${BIN_DIR}/dimclaw"
echo "DimClaw 安装完成 "${BIN_DIR}/dimclaw""
