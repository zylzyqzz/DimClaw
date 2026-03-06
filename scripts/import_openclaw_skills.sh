#!/bin/bash
set -e

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"

TMP_DIR="${ROOT}/.tmp_openclaw"
rm -rf "$TMP_DIR"
git clone --depth=1 https://github.com/VoltAgent/awesome-openclaw-skills.git "$TMP_DIR"

mkdir -p skills/custom
for skill_dir in "$TMP_DIR"/skills/*; do
  if [ -d "$skill_dir" ]; then
    skill_name=$(basename "$skill_dir")
    if [ -f "$skill_dir/SKILL.md" ]; then
      python3 scripts/openclaw_to_dimclaw.py "$skill_dir/SKILL.md" > "skills/custom/${skill_name}.toml"
      echo "导入技能: $skill_name"
    fi
  fi
done

echo "导入完成"
