# DimClaw Ubuntu 22.04 部署指南（V0.1）

## 1. 目录与运行模型

推荐目录：
- 程序目录：`/opt/dimclaw`
- 数据目录：`/var/lib/dimclaw`
- 日志目录：`/var/log/dimclaw`

服务由 systemd 常驻运行，默认以普通用户 `dimclaw` 运行（不是 root）。

## 2. 配置覆盖（环境变量）

当前运行时支持环境变量：
- `DIMCLAW_DATA_DIR`：任务与调度数据目录
- `DIMCLAW_LOG_DIR`：日志目录（空字符串表示禁用文件日志，只输出 stdout）
- `DIMCLAW_MAX_RETRIES`：最大重试次数
- `DIMCLAW_POLL_INTERVAL_MS`：运行时轮询间隔

示例见：`configs/runtime.example.toml`（用于复制变量，不是解析型配置文件）。

## 3. 一键验收（本地）

```bash
cargo build --release
./target/release/dimclaw submit --title "验收任务" --command "echo dimclaw_ok" --timeout-secs 10
./target/release/dimclaw run --once
./target/release/dimclaw list
```

预期：能看到任务从 `pending -> planning -> running -> verifying -> success`。

## 4. schedule + 服务模式演示（10 秒内可见）

```bash
./target/release/dimclaw schedule --title "5秒定时" --interval-secs 5 --command "echo scheduled_ok" --timeout-secs 10
sudo bash scripts/deploy_ubuntu.sh
sleep 10
sudo journalctl -u dimclaw -n 100 --no-pager
sudo tail -n 100 /var/log/dimclaw/dimclaw.log
```

预期：日志中出现 `已投递 task_id=...` 与 `status=success`。

## 5. 更新与回滚

- 更新：重新 `cargo build --release`，再执行 `sudo bash scripts/deploy_ubuntu.sh`。
- 脚本会把新版本放到 `/opt/dimclaw/releases/<timestamp>/dimclaw`，并把 `/opt/dimclaw/current` 切换到新版本。
- 回滚：手工把 `current` 软链切回旧版本后 `sudo systemctl restart dimclaw`。

## 6. 卸载/回滚脚本

```bash
# 默认仅停服务并删除 systemd unit，保留程序/数据/日志
sudo bash scripts/undeploy_ubuntu.sh

# 彻底删除（谨慎）
sudo DIMCLAW_PURGE_INSTALL=1 DIMCLAW_PURGE_DATA=1 DIMCLAW_PURGE_LOGS=1 bash scripts/undeploy_ubuntu.sh
```

## 7. 体积说明（重点）

`target/` 不是源码本体，而是 Rust 编译产物和依赖缓存，所以通常很大。

### 7.1 源码体积（排除 target/.git/data/logs）

```bash
find . -type f \
  -not -path "./target/*" \
  -not -path "./.git/*" \
  -not -path "./data/*" \
  -not -path "./logs/*" \
  -printf "%s\n" | awk '{s+=$1} END{print "source_bytes="s}'
```

### 7.2 release 二进制体积

```bash
ls -lh ./target/release/dimclaw
# 或
stat -c '%n %s bytes' ./target/release/dimclaw
```

### 7.3 target 为什么大、如何清理

- 大的原因：依赖编译缓存、中间目标文件、调试信息。
- 清理命令：`cargo clean`
- 影响：只清缓存，不影响已经部署在 `/opt/dimclaw/current/dimclaw` 的运行服务；下次本地编译会重新耗时构建。

## 8. 权限与安全建议

- 默认普通用户运行，降低风险。
- `shell_command` 具备执行系统命令能力，建议：
  - 仅信任本地任务来源
  - 用受限用户运行服务
  - 控制 `DIMCLAW_DATA_DIR` 写权限
- 若必须 root 运行，可在 systemd unit 中改 `User=root`（不推荐）。
