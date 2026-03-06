#!/usr/bin/env python3
import sys
from pathlib import Path

p = Path(sys.argv[1])
name = p.parent.name.replace(' ', '_')
content = p.read_text(encoding='utf-8', errors='ignore')
desc = content.splitlines()[0].strip('# ').strip() if content else name

print(f'name = "{name}"')
print(f'description = "{desc}"')
print('exec_type = "shell"')
print('params_schema = { type = "object" }')
print('command_template = "echo imported skill"')
print('method = "GET"')
print('url = ""')
print('body_template = ""')
print('timeout_secs = 20')
