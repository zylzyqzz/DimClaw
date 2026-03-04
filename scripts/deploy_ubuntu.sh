#!/usr/bin/env bash
set -euo pipefail

SERVICE_NAME="dimclaw"
INSTALL_DIR="${DIMCLAW_INSTALL_DIR:-/opt/dimclaw}"
DATA_DIR="${DIMCLAW_DATA_DIR:-/var/lib/dimclaw}"
LOG_DIR="${DIMCLAW_LOG_DIR:-/var/log/dimclaw}"
SERVICE_USER="${DIMCLAW_SERVICE_USER:-dimclaw}"
CREATE_USER="${DIMCLAW_CREATE_USER:-1}"
STRIP_BIN="${DIMCLAW_STRIP:-1}"
BIN_SRC="${DIMCLAW_BIN_SRC:-./target/release/dimclaw}"

if [[ "${EUID}" -ne 0 ]]; then
  echo "请使用 root 或 sudo 运行部署脚本。"
  exit 1
fi

if [[ ! -x "${BIN_SRC}" ]]; then
  echo "未找到可执行文件: ${BIN_SRC}"
  echo "请先执行: cargo build --release"
  exit 1
fi

if [[ "${CREATE_USER}" == "1" ]]; then
  if ! id -u "${SERVICE_USER}" >/dev/null 2>&1; then
    useradd --system --home "${INSTALL_DIR}" --shell /usr/sbin/nologin "${SERVICE_USER}"
  fi
fi

mkdir -p "${INSTALL_DIR}/releases" "${DATA_DIR}" "${LOG_DIR}"
release_tag="$(date +%Y%m%d%H%M%S)"
release_dir="${INSTALL_DIR}/releases/${release_tag}"
mkdir -p "${release_dir}"
install -m 0755 "${BIN_SRC}" "${release_dir}/dimclaw"

if [[ "${STRIP_BIN}" == "1" ]] && command -v strip >/dev/null 2>&1; then
  strip "${release_dir}/dimclaw" || true
fi

ln -sfn "${release_dir}" "${INSTALL_DIR}/current"
chown -R "${SERVICE_USER}:${SERVICE_USER}" "${INSTALL_DIR}" "${DATA_DIR}" "${LOG_DIR}"

cat >/etc/systemd/system/${SERVICE_NAME}.service <<EOF
[Unit]
Description=DimClaw Local Multi-Agent Runtime
After=network.target

[Service]
Type=simple
User=${SERVICE_USER}
Group=${SERVICE_USER}
WorkingDirectory=${INSTALL_DIR}/current
Environment=DIMCLAW_DATA_DIR=${DATA_DIR}
Environment=DIMCLAW_LOG_DIR=${LOG_DIR}
Environment=DIMCLAW_MAX_RETRIES=3
ExecStart=${INSTALL_DIR}/current/dimclaw run --with-scheduler
Restart=always
RestartSec=2
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=full
ProtectHome=true
ReadWritePaths=${DATA_DIR} ${LOG_DIR}

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now ${SERVICE_NAME}
systemctl --no-pager status ${SERVICE_NAME} || true

echo "部署完成:"
echo "  二进制: ${INSTALL_DIR}/current/dimclaw"
echo "  数据目录: ${DATA_DIR}"
echo "  日志目录: ${LOG_DIR}"
echo "  服务名: ${SERVICE_NAME}"
