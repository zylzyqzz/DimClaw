# DimClaw

## 1. 项目简介

DimClaw 是一个使用 Rust 构建的本地任务执行框架，核心定位是：
- 以任务状态机驱动执行流程；
- 通过 Planner / Executor / Verifier / Recovery 四阶段协同处理任务；
- 支持通过技能（Skills）调用系统能力；
- 提供 CLI 与 Web API 两种操作入口。

项目当前以“可运行、可迭代”为目标，强调本地可控与工程化扩展能力。

---

## 2. 当前状态（v0.4.0，开发中）

- 当前包版本：`v0.4.0`（见 `Cargo.toml`）。
- 项目已具备端到端最小闭环：提交任务 → 运行状态机 → 执行技能 → 校验/重试 → 持久化。
- Web 端已提供静态页面入口和一组可用 API。
- 同时仍存在若干“开发中/预留”模块，见下文“开发中/已预留模块”。

---

## 3. 已实现能力

### 3.1 CLI 与运行时

已实现命令：
- `submit`：提交任务
- `run`：运行任务主循环（支持 `--once`、`--with-scheduler`）
- `list`：列出任务
- `schedule`：注册 interval 定时任务
- `doctor`：检查运行配置与模型配置
- `server`：启动 Web 服务

运行时能力：
- 任务状态机：`pending -> planning -> running -> verifying -> retrying -> success/failed/cancelled`
- 启动时回捞未完成任务并入队
- 支持 Ctrl+C 取消与安全退出
- 任务与调度数据本地持久化（JSON）

### 3.2 Agent 执行链路

已实现四阶段 Agent：
- `PlannerAgent`
- `ExecutorAgent`
- `VerifierAgent`
- `RecoveryAgent`

并支持按阶段执行自定义 Agent（`before_xxx` / `after_xxx`）。

### 3.3 Provider（模型接入）

已实现 `OpenAiCompatibleProvider`，包括：
- `/chat/completions` 请求
- 超时、重试、退避
- 取消控制与响应解析

### 3.4 Skills（技能）

已注册并可调用的内置技能包含以下类别：

- 命令执行：`shell_command`、`script_execute`
- 文件处理：`file_read`、`file_write`、`file_list`、`file_move`、`file_copy`、`file_delete`
- 网络请求：`http_request`
- 进程/系统：`process_list`、`process_kill`、`system_monitor`、`service_control`、`schedule_task`
- 媒体工具封装：`yt_dlp`、`ffmpeg`、`whisper`
- 浏览器相关：`browser_automator`、`browser_open`、`browser_screenshot`、`browser_click`、`browser_fill`

> 说明：浏览器相关能力并非全部完整自动化；部分实现目前仍为基础态或占位态（例如 `browser_click`、`browser_fill`）。

### 3.5 Web API / 页面入口

`server` 启动后，已提供：
- 仪表盘与页面入口（静态资源）
- 任务、配置、技能、插件、聊天等 API
- 插件状态接口与连接状态检查接口

### 3.6 Feishu 运行态管理（插件侧）

已包含 Feishu sidecar 运行态管理代码（安装状态、进程状态、启停、重启监控、日志缓冲等）。

---

## 4. 开发中/已预留模块

以下模块已存在目录或接口，但完成度仍有限：

- `ui/`：模块已预留，当前仅保留初始化入口
- `adapters/`：模块已预留，当前仅保留初始化入口
- `channels/`：含 Feishu/Telegram/Webhook 结构与基础行为，整体仍偏基础实现
- `memory/`：已提供模块与实现文件，尚未形成完整生产化记忆链路
- `providers/`：主用链路以 OpenAI Compatible 为主，其他抽象与兜底能力仍在演进
- `agents/hands`：已具备框架与若干示例能力，整体仍在迭代中

---

## 5. 项目结构说明

```text
DimClaw/
├── src/                 # 程序入口（CLI）
├── core/                # 运行时内核（状态机、任务、存储、API）
├── agents/              # 四阶段 Agent + 自定义 Agent + hands
├── skills/              # 技能注册与实现
├── providers/           # 模型 Provider
├── plugins/             # 插件管理与安装流程
├── feishu/              # Feishu sidecar 运行态管理
├── channels/            # 渠道模块（基础实现）
├── memory/              # 记忆模块（开发中）
├── configs/             # 配置模型与配置文件
├── scheduler/           # 定时任务投递
├── tests/               # 测试
└── scripts/             # 脚本工具
```

---

## 6. 快速开始

### 6.1 构建

```bash
cargo build
```

### 6.2 查看帮助

```bash
cargo run -- --help
```

### 6.3 启动 Web 服务

```bash
cargo run -- server --host 127.0.0.1 --port 8080
```

---

## 7. CLI 命令示例

### 提交任务

```bash
cargo run -- submit --title "demo" --command "echo hello" --timeout-secs 10
```

### 执行一次主循环

```bash
cargo run -- run --once
```

### 查看任务列表

```bash
cargo run -- list
```

### 注册定时任务

```bash
cargo run -- schedule --title "tick" --interval-secs 30 --command "echo tick" --timeout-secs 10
```

### 配置诊断

```bash
cargo run -- doctor
```

---

## 8. 路线图

结合当前代码完成度，后续按以下节奏推进（以“先稳后扩”为原则）：

### 阶段 A：稳定性收口（当前优先）

1. 修复并稳定测试基线（优先处理 `server startup timeout` 相关用例）。
2. 收敛告警与工程规范（`cargo fmt` / `clippy` / CI 门禁一致）。
3. 持续保持“代码现状—README—版本号”同步更新。

### 阶段 B：把“占位能力”变成可用能力

1. 浏览器技能从基础态升级到可验证的自动化流程（open/click/fill/screenshot）。
2. `agents/hands` 从示例能力升级为可配置、可观测、可复用执行单元。
3. 完善错误分类与审计输出，降低排障成本。

### 阶段 C：补齐模块化能力

1. `channels`：从基础结构推进到真实通道交互链路。
2. `memory`：完善检索与写入链路，形成可用记忆能力。
3. `adapters/ui`：从预留入口推进到实际可用模块。

> 说明：以上路线图只描述已存在模块的推进方向，不代表这些能力已全部完成。

---

## 9. License

MIT
