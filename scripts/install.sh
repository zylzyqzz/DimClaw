#!/bin/bash
set -e

echo "正在安装 DimClaw..."

OS=$(uname -s)
ARCH=$(uname -m)
case "$OS" in
  Linux) FILENAME="dimclaw-linux-$ARCH" ;;
  Darwin) FILENAME="dimclaw-macos-$ARCH" ;;
  *) echo "不支持的系统: $OS"; exit 1 ;;
esac

URL="https://github.com/zylzyqzz/DimClaw/releases/latest/download/$FILENAME"
curl -fsSL "$URL" -o dimclaw
chmod +x dimclaw
sudo mv dimclaw /usr/local/bin/

echo "DimClaw 已安装到 /usr/local/bin/dimclaw"
echo "运行 'dimclaw server' 启动服务"
