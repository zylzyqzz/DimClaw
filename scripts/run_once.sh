#!/usr/bin/env bash
set -euo pipefail

BIN="${DIMCLAW_BIN:-./target/release/dimclaw}"

if [[ ! -x "${BIN}" ]]; then
  echo "未找到可执行文件: ${BIN}"
  echo "请先执行 cargo build --release"
  exit 1
fi

${BIN} submit --title "run_once_验收" --command "echo run_once_ok" --timeout-secs 10
${BIN} run --once
${BIN} list
