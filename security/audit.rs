use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use chrono::Local;

use crate::core::logger;

pub fn log_audit(action: &str, target: &str, detail: &str, allowed: bool) {
    let status = if allowed { "ALLOW" } else { "DENY" };
    let line = format!(
        "{} [{}] action={} target={} detail={}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        status,
        action,
        target,
        detail.replace('\n', "\\n")
    );

    logger::log(format!("[审计] {} {} {}", status, action, target));

    let path = Path::new("./logs/audit.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
}

