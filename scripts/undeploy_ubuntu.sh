#!/usr/bin/env bash
set -euo pipefail

SERVICE_NAME="dimclaw"
INSTALL_DIR="${DIMCLAW_INSTALL_DIR:-/opt/dimclaw}"
DATA_DIR="${DIMCLAW_DATA_DIR:-/var/lib/dimclaw}"
LOG_DIR="${DIMCLAW_LOG_DIR:-/var/log/dimclaw}"
PURGE_INSTALL="${DIMCLAW_PURGE_INSTALL:-0}"
PURGE_DATA="${DIMCLAW_PURGE_DATA:-0}"
PURGE_LOGS="${DIMCLAW_PURGE_LOGS:-0}"

if [[ "${EUID}" -ne 0 ]]; then
  echo "请使用 root 或 sudo 运行卸载脚本。"
  exit 1
fi

if systemctl list-unit-files | grep -q "^${SERVICE_NAME}\.service"; then
  systemctl disable --now ${SERVICE_NAME} || true
fi

rm -f /etc/systemd/system/${SERVICE_NAME}.service
systemctl daemon-reload
systemctl reset-failed || true

if [[ "${PURGE_INSTALL}" == "1" ]]; then
  rm -rf "${INSTALL_DIR}"
fi
if [[ "${PURGE_DATA}" == "1" ]]; then
  rm -rf "${DATA_DIR}"
fi
if [[ "${PURGE_LOGS}" == "1" ]]; then
  rm -rf "${LOG_DIR}"
fi

echo "卸载完成。"
echo "保留策略: install=${PURGE_INSTALL} data=${PURGE_DATA} logs=${PURGE_LOGS}"
