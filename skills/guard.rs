use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

use crate::configs::load_security;
use crate::security::audit::log_audit;

pub fn unrestricted_mode() -> bool {
    load_security().map(|s| s.unrestricted_mode).unwrap_or(false)
}

pub fn workspace_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    Ok(cwd.canonicalize().unwrap_or(cwd))
}

pub fn ensure_path_allowed(raw: &str, action: &str) -> Result<PathBuf> {
    if raw.trim().is_empty() {
        log_audit(action, raw, "empty path", false);
        return Err(anyhow!("path 不能为空"));
    }

    let path = PathBuf::from(raw);
    if unrestricted_mode() {
        log_audit(action, raw, "unrestricted_mode=true", true);
        return Ok(path);
    }

    let root = workspace_root()?;
    let candidate = canonicalize_for_access(&path)?;
    if !candidate.starts_with(&root) {
        log_audit(
            action,
            raw,
            &format!("outside workspace root={}", root.display()),
            false,
        );
        return Err(anyhow!("路径超出工作区，请在 security.toml 开启 unrestricted_mode"));
    }

    log_audit(action, raw, "workspace allowed", true);
    Ok(path)
}

fn canonicalize_for_access(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return Ok(path.canonicalize()?);
    }

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    if let Some(parent) = absolute.parent() {
        let parent_abs = if parent.exists() {
            parent.canonicalize()?
        } else {
            std::env::current_dir()?.canonicalize()?
        };
        if let Some(name) = absolute.file_name() {
            return Ok(parent_abs.join(name));
        }
    }

    Ok(absolute)
}

pub fn audit_command(action: &str, command: &str, allowed: bool, detail: &str) {
    log_audit(action, command, detail, allowed);
}

