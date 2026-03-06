use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

use crate::agents::hands::{builtin_hands, Condition, Hand, HandResult};

#[derive(Clone, Debug, Serialize)]
pub struct HandStatus {
    pub name: String,
    pub description: String,
    pub last_run: Option<String>,
    pub next_run: Option<String>,
    pub paused: bool,
    pub last_output: String,
    pub running: bool,
}

#[derive(Clone)]
pub struct HandScheduler {
    pub hands: Arc<HashMap<String, Arc<dyn Hand>>>,
    pub last_run: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
    pub next_run: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
    pub paused: Arc<Mutex<HashMap<String, bool>>>,
    pub last_output: Arc<Mutex<HashMap<String, String>>>,
    pub running: Arc<AtomicBool>,
}

impl Default for HandScheduler {
    fn default() -> Self {
        let mut map: HashMap<String, Arc<dyn Hand>> = HashMap::new();
        for hand in builtin_hands() {
            map.insert(hand.name().to_string(), hand);
        }
        let now = Utc::now();
        let mut next = HashMap::new();
        for (name, hand) in &map {
            let ts = calc_next(now, hand.schedule().interval, hand.schedule().cron.as_deref());
            next.insert(name.clone(), ts);
        }
        Self {
            hands: Arc::new(map),
            last_run: Arc::new(Mutex::new(HashMap::new())),
            next_run: Arc::new(Mutex::new(next)),
            paused: Arc::new(Mutex::new(HashMap::new())),
            last_output: Arc::new(Mutex::new(HashMap::new())),
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl HandScheduler {
    pub fn start(&self) -> impl std::future::Future<Output = Result<()>> + Send + 'static {
        let this = self.clone();
        async move {
            if this.running.swap(true, Ordering::SeqCst) {
                return Ok(());
            }
            tokio::spawn(async move {
                loop {
                    if !this.running.load(Ordering::SeqCst) {
                        break;
                    }
                    let _ = this.tick().await;
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            });
            Ok(())
        }
    }

    async fn tick(&self) -> Result<()> {
        let now = Utc::now();
        for (name, hand) in self.hands.iter() {
            if self.is_paused(name) {
                continue;
            }
            let due = self
                .next_run
                .lock()
                .ok()
                .and_then(|m| m.get(name).cloned())
                .map(|t| t <= now)
                .unwrap_or(false);
            if !due {
                continue;
            }
            if !condition_ok(hand.as_ref()) {
                self.set_next(name, calc_next(now, hand.schedule().interval, hand.schedule().cron.as_deref()));
                continue;
            }
            let _ = self.trigger_now(name).await;
        }
        Ok(())
    }

    pub async fn trigger_now(&self, name: &str) -> Result<HandResult> {
        let hand = self
            .hands
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("hand not found: {}", name))?
            .clone();
        let out = hand.execute().await?;
        let now = Utc::now();
        if let Ok(mut m) = self.last_run.lock() {
            m.insert(name.to_string(), now);
        }
        if let Ok(mut m) = self.last_output.lock() {
            m.insert(name.to_string(), out.output.clone());
        }
        self.set_next(
            name,
            calc_next(now, hand.schedule().interval, hand.schedule().cron.as_deref()),
        );
        Ok(out)
    }

    pub fn get_status(&self) -> Vec<HandStatus> {
        let mut out = Vec::new();
        for (name, hand) in self.hands.iter() {
            let last = self
                .last_run
                .lock()
                .ok()
                .and_then(|m| m.get(name).cloned())
                .map(|v| v.to_rfc3339());
            let next = self
                .next_run
                .lock()
                .ok()
                .and_then(|m| m.get(name).cloned())
                .map(|v| v.to_rfc3339());
            let paused = self.is_paused(name);
            let last_output = self
                .last_output
                .lock()
                .ok()
                .and_then(|m| m.get(name).cloned())
                .unwrap_or_default();
            out.push(HandStatus {
                name: name.clone(),
                description: hand.description().to_string(),
                last_run: last,
                next_run: next,
                paused,
                last_output,
                running: self.running.load(Ordering::SeqCst) && !paused,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub fn pause(&self, name: &str) {
        if let Ok(mut m) = self.paused.lock() {
            m.insert(name.to_string(), true);
        }
    }

    pub fn resume(&self, name: &str) {
        if let Ok(mut m) = self.paused.lock() {
            m.insert(name.to_string(), false);
        }
    }

    fn is_paused(&self, name: &str) -> bool {
        self.paused
            .lock()
            .ok()
            .and_then(|m| m.get(name).cloned())
            .unwrap_or(false)
    }

    fn set_next(&self, name: &str, value: DateTime<Utc>) {
        if let Ok(mut m) = self.next_run.lock() {
            m.insert(name.to_string(), value);
        }
    }
}

fn calc_next(now: DateTime<Utc>, interval: Option<u64>, cron: Option<&str>) -> DateTime<Utc> {
    if let Some(sec) = interval {
        return now + Duration::seconds(sec as i64);
    }
    if let Some(expr) = cron {
        if let Some(minutes) = parse_cron_minutes(expr) {
            return now + Duration::minutes(minutes as i64);
        }
    }
    now + Duration::minutes(10)
}

fn parse_cron_minutes(expr: &str) -> Option<u64> {
    let first = expr.split_whitespace().next()?.trim();
    if let Some(v) = first.strip_prefix("*/") {
        return v.parse::<u64>().ok().filter(|n| *n > 0);
    }
    if first == "0" {
        return Some(60);
    }
    None
}

fn condition_ok(hand: &dyn Hand) -> bool {
    match hand.schedule().condition {
        Some(Condition::FileExists(path)) => path.exists(),
        Some(Condition::ProcessRunning(_)) => true,
        Some(Condition::CpuAbove(_)) => true,
        Some(Condition::MemoryAbove(_)) => true,
        Some(Condition::Custom(_)) => true,
        None => true,
    }
}
