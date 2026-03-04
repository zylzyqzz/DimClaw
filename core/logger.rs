use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use chrono::Local;

#[derive(Default)]
struct LoggerState {
    file: Option<File>,
}

static LOGGER: OnceLock<Mutex<LoggerState>> = OnceLock::new();

pub fn init(log_dir: Option<PathBuf>) {
    let state = LOGGER.get_or_init(|| Mutex::new(LoggerState::default()));
    let mut guard = state.lock().expect("logger mutex poisoned");
    if let Some(dir) = log_dir {
        if fs::create_dir_all(&dir).is_ok() {
            let log_path = dir.join("dimclaw.log");
            guard.file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(log_path)
                .ok();
        }
    }
}

pub fn log(msg: impl AsRef<str>) {
    let line = format!(
        "{} {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        msg.as_ref()
    );
    print!("{line}");
    if let Some(state) = LOGGER.get() {
        if let Ok(mut guard) = state.lock() {
            if let Some(file) = guard.file.as_mut() {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }
}
