use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tokio_util::sync::CancellationToken;

#[path = "../agents/mod.rs"]
mod agents;
#[path = "../adapters/mod.rs"]
mod adapters;
#[path = "../configs/mod.rs"]
mod configs;
#[path = "../core/mod.rs"]
mod core;
#[path = "../providers/mod.rs"]
mod providers;
#[path = "../scheduler/mod.rs"]
mod scheduler;
#[path = "../skills/mod.rs"]
mod skills;
#[path = "../ui/mod.rs"]
mod ui;

use configs::RuntimeConfig;
use core::logger;
use core::runtime::Runtime;
use core::storage::TaskStorage;
use core::task::Task;
use scheduler::SchedulerStore;

#[derive(Parser, Debug)]
#[command(name = "dimclaw", about = "DimClaw 本地多智能体执行框架 V0.1")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    #[command(about = "创建一个测试任务")]
    Submit {
        #[arg(long, default_value = "最小测试任务")]
        title: String,
        #[arg(long, default_value = "echo DimClaw V0.1")]
        command: String,
        #[arg(long, default_value_t = 10)]
        timeout_secs: u64,
    },
    #[command(about = "启动运行时主循环")]
    Run {
        #[arg(long, default_value_t = false)]
        with_scheduler: bool,
        #[arg(long, default_value_t = false)]
        once: bool,
    },
    #[command(about = "查看任务列表")]
    List,
    #[command(about = "注册 interval 定时任务")]
    Schedule {
        #[arg(long)]
        title: String,
        #[arg(long)]
        interval_secs: u64,
        #[arg(long)]
        command: String,
        #[arg(long, default_value_t = 10)]
        timeout_secs: u64,
    },
    #[command(about = "检查运行配置与模型配置")]
    Doctor,
}

#[tokio::main]
async fn main() -> Result<()> {
    adapters::init();
    ui::init();

    let cli = Cli::parse();
    let config = RuntimeConfig::load().unwrap_or_default();
    logger::init(config.log_dir.clone());
    logger::log(format!(
        "[系统] 启动配置 data_dir={} log_dir={} max_retries={} poll_interval_ms={}",
        config.data_dir_display(),
        config.log_dir_display(),
        config.max_retries,
        config.poll_interval_ms
    ));

    let storage = Arc::new(TaskStorage::new(config.data_dir.clone()));
    storage.ensure_dirs().await?;
    let scheduler_store = SchedulerStore::new(config.data_dir.clone());

    match cli.command {
        Commands::Submit {
            title,
            command,
            timeout_secs,
        } => {
            let payload = build_task_payload(command, timeout_secs);
            let task = Task::new(title, payload);
            storage.create_task(&task).await?;
            logger::log(format!(
                "[提交] 已创建任务 id={} status={}（待 run 时入队执行）",
                task.id, task.status
            ));
        }
        Commands::List => {
            let mut tasks = storage.list_tasks().await?;
            tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));
            logger::log(format!("[列表] 任务总数: {}", tasks.len()));
            for t in tasks {
                logger::log(format!(
                    "- id={} status={} retry={} step={} agent={} updated={} title={}",
                    t.id,
                    t.status,
                    t.retry_count,
                    t.step,
                    t.current_agent.clone().unwrap_or_else(|| "-".to_string()),
                    t.updated_at,
                    t.title
                ));
            }
        }
        Commands::Schedule {
            title,
            interval_secs,
            command,
            timeout_secs,
        } => {
            let payload = build_task_payload(command, timeout_secs);
            let entry = scheduler::ScheduleEntry::new(title, interval_secs, payload);
            scheduler_store.ensure_dir().await?;
            scheduler_store.add_schedule(entry.clone()).await?;
            logger::log(format!(
                "[定时] 注册成功 id={} interval={}s title={}",
                entry.id, entry.interval_secs, entry.title
            ));
        }
        Commands::Run {
            with_scheduler,
            once,
        } => {
            let cancel = CancellationToken::new();
            let mut runtime = Runtime::new(config.clone(), storage.clone(), cancel.clone()).await?;
            runtime.bootstrap_enqueue_unfinished().await?;

            let scheduler_handle = if with_scheduler {
                scheduler_store.ensure_dir().await?;
                let entries = scheduler_store.list_schedules().await?;
                Some(scheduler::spawn_scheduler(
                    entries,
                    runtime.queue_sender(),
                    storage.clone(),
                    cancel.clone(),
                ))
            } else {
                None
            };

            let ctrl_cancel = cancel.clone();
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    logger::log("[系统] 收到 Ctrl+C，开始安全停止...");
                    ctrl_cancel.cancel();
                }
            });

            runtime.run_loop(once).await?;
            if let Some(handle) = scheduler_handle {
                let _ = handle.await;
            }
            logger::log("[系统] 运行时已停止");
        }
        Commands::Doctor => {
            run_doctor(&config);
        }
    }

    Ok(())
}

fn build_task_payload(command: String, timeout_secs: u64) -> serde_json::Value {
    serde_json::json!({
        "input": {
            "command": command,
            "timeout_secs": timeout_secs
        }
    })
}

fn run_doctor(config: &RuntimeConfig) {
    logger::log("[Doctor] 开始检查");
    logger::log(format!(
        "[Doctor] 配置文件: {}",
        if config.config_exists() { "存在" } else { "不存在（将使用默认配置）" }
    ));
    logger::log(format!(
        "[Doctor] LLM 开关: {}",
        if config.llm.enabled { "启用" } else { "关闭" }
    ));

    match config.selected_provider() {
        Ok(provider) => {
            logger::log(format!("[Doctor] provider_name: {}", provider.provider_name));
            logger::log(format!("[Doctor] protocol: {}", provider.protocol));
            logger::log(format!("[Doctor] base_url: {}", provider.base_url));
            logger::log(format!("[Doctor] model: {}", provider.model));
            logger::log(format!(
                "[Doctor] api_key: {}",
                if provider.api_key.trim().is_empty() || provider.api_key.trim() == "YOUR_API_KEY" {
                    "为空"
                } else {
                    "已配置"
                }
            ));
            if provider.base_url.trim().is_empty()
                || provider.model.trim().is_empty()
                || provider.api_key.trim().is_empty()
                || provider.api_key.trim() == "YOUR_API_KEY"
            {
                logger::log("[Doctor] 检查结果: 异常（请补全 base_url/model/api_key）");
            } else {
                logger::log("[Doctor] 检查结果: 正常");
            }
        }
        Err(e) => {
            logger::log(format!("[Doctor] 检查结果: 异常，{}", e));
        }
    }
}
