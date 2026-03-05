# DimClaw

<div align="center">

**一个本地优先、超轻量、多智能体协作的执行框架**

[![CI](https://github.com/zylzyqzz/DimClaw/actions/workflows/ci.yml/badge.svg)](https://github.com/zylzyqzz/DimClaw/actions/workflows/ci.yml)
[![Release](https://github.com/zylzyqzz/DimClaw/actions/workflows/release.yml/badge.svg)](https://github.com/zylzyqzz/DimClaw/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

</div>

## 📖 简介

**DimClaw** 是一个 **本地优先**、**超轻量** 的 **多智能体执行框架**。它的核心是一个可恢复、可重试的任务运行时，围绕“任务状态机 + 四智能体分工 + 技能插件”设计。

你可以把它看作 **AI 自动化的底层操作系统**——当前版本（v0.1）已完成最小闭环，后续将逐步接入大模型、通信通道、更多技能，让智能体真正“能思考、会干活”。

## ✨ 核心特性

- **本地优先**：所有数据、调度、执行均在本地，不强制依赖云服务。
- **超轻量**：Rust 实现，Release 二进制仅 1~2 MB，源码仓库小于 100 KB。
- **任务状态机**：任务有 8 个明确状态（pending → planning → running → verifying → retrying → success/failed/cancelled），状态流转集中管理。
- **四智能体协作**：
  - **Planner**：任务规划
  - **Executor**：执行任务（调用技能）
  - **Verifier**：验证结果
  - **Recovery**：失败重试 / 恢复
- **技能系统**：内置 `shell_command`（可执行系统命令），支持超时、中断；未来可扩展更多技能。
- **定时任务**：支持 interval 定时投递任务。
- **持久化**：任务状态实时保存到本地 JSON 文件，系统崩溃后可恢复。
- **一键安装**：提供 Linux/macOS/Windows 一键安装脚本（从 GitHub Release 下载二进制）。
- **CI/CD**：GitHub Actions 自动构建、测试、发布三平台二进制。

## 🚀 快速开始

### 方式一：从源码编译（需要 Rust 环境）

```bash
git clone https://github.com/zylzyqzz/DimClaw.git
cd DimClaw
cargo build --release
./target/release/dimclaw --help
```

### 方式二：一键安装（推荐）

#### Linux / macOS
```bash
curl -fsSL https://raw.githubusercontent.com/zylzyqzz/DimClaw/main/scripts/install.sh | bash
# 安装后二进制位于 ~/.local/bin/dimclaw
dimclaw --help
```

#### Windows PowerShell
```powershell
iwr -useb https://raw.githubusercontent.com/zylzyqzz/DimClaw/main/scripts/install.ps1 | iex
# 安装后二进制位于 %USERPROFILE%\.dimclaw\bin\dimclaw.exe
& "$env:USERPROFILE\.dimclaw\bin\dimclaw.exe" --help
```

## 📦 使用示例

### 提交一个任务（执行 shell 命令）

```bash
dimclaw submit --title "测试任务" --command "echo hello_dimclaw" --timeout-secs 10
# 输出任务 ID，例如：2a9f3e1b-7c8d-4e5f-9a0b-1c2d3e4f5g6h
```

### 单次运行（处理当前队列中的任务）

```bash
dimclaw run --once
# 你会看到任务状态流转：pending → planning → running → verifying → success
```

### 查看任务列表

```bash
dimclaw list
```

输出示例：
```
ID                                   TITLE         STATUS    STEP  RETRY  ERROR
2a9f3e1b-7c8d-4e5f-9a0b-1c2d3e4f5g6h 测试任务      success   4     0      None
```

### 注册一个定时任务（每 30 秒执行一次）

```bash
dimclaw schedule --title "定时任务" --interval-secs 30 --command "echo scheduled_ok" --timeout-secs 10
```

### 启动运行时（常驻，并开启定时投递）

```bash
dimclaw run --with-scheduler
```

## ⚙️ 配置

配置文件位于 `configs/runtime.toml`（首次运行会自动生成示例）。支持以下配置项：

```toml
[runtime]
data_dir = "./data"      # 任务持久化目录
log_dir = "./logs"       # 日志目录
max_retries = 3          # 默认最大重试次数

[llm]
enabled = false          # 模型接入开关（V0.2 后可用）
provider = "default"

[providers.default]
protocol = "openai_compatible"
provider_name = "nvidia"
base_url = "https://integrate.api.nvidia.com/v1"
api_key = "YOUR_API_KEY"
model = "nvidia/qwen/qwen3.5-397b-a17b"
timeout_secs = 60
max_tokens = 2048
temperature = 0.2
```

## 🔧 构建与发布

本项目使用 GitHub Actions 自动构建和发布：

- 每次推送到 `main` 分支，执行 `CI`（`cargo build` + `cargo test`）。
- 每次推送形如 `v*` 的标签，自动构建 Linux、Windows、macOS 二进制并发布到 Releases。

手动构建 Release 版本：

```bash
cargo build --release
strip target/release/dimclaw   # （可选）减小体积
ls -lh target/release/dimclaw*
```

## 📁 项目结构

```
dimclaw/
├── .github/workflows/    # GitHub Actions 配置
├── core/                 # 运行时内核（状态机、任务、队列、存储）
├── agents/               # 四智能体实现
├── skills/               # 技能系统（内置 shell_command）
├── scheduler/            # 定时任务模块
├── configs/              # 配置文件
├── scripts/              # 一键安装脚本
├── src/                  # 主程序入口
├── data/                 # 任务数据（运行时生成）
├── logs/                 # 日志（运行时生成）
├── Cargo.toml
└── README.md
```

## 🤝 贡献

欢迎任何形式的贡献！如果你有好的想法、发现了 bug，欢迎提交 Issue 或 Pull Request。

## 📄 许可

本项目基于 [MIT 许可证](LICENSE) 开源。

## 🌟 未来计划

- **V0.2**：接入大模型，让智能体真正“会思考”。
- **V0.3**：飞书通道插件化，支持在聊天工具中派单。
- **V0.4**：断点续跑、幂等、审计日志，强化执行可靠性。
- **V1.0**：技能插件市场、工作流 DAG、Agent 集群。
