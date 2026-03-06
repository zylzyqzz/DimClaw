use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Result};

pub async fn download_and_extract(url: &str, ext: &str, output_dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(output_dir)?;

    let client = reqwest::Client::new();
    let resp = client.get(url).send().await.map_err(|e| anyhow!("下载失败: {}", e))?;
    if !resp.status().is_success() {
        return Err(anyhow!("下载失败: HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().await?;

    let normalized = if ext.trim().is_empty() {
        if url.ends_with(".zip") {
            "zip"
        } else if url.ends_with(".tar.gz") || url.ends_with(".tgz") {
            "tar.gz"
        } else {
            "bin"
        }
    } else {
        ext
    };

    let tmp_path = output_dir.join(match normalized {
        "zip" => "plugin.zip",
        "tar.gz" | "tgz" => "plugin.tar.gz",
        _ => "plugin.bin",
    });
    std::fs::write(&tmp_path, &bytes)?;

    match normalized {
        "zip" => extract_zip_with_system(&tmp_path, output_dir)?,
        "tar.gz" | "tgz" => extract_targz_with_system(&tmp_path, output_dir)?,
        _ => {
            let file_name = file_name_from_url(url).unwrap_or_else(|| "plugin.bin".to_string());
            std::fs::copy(&tmp_path, output_dir.join(file_name))?;
        }
    }

    let _ = std::fs::remove_file(tmp_path);
    Ok(output_dir.to_path_buf())
}

fn extract_zip_with_system(archive: &Path, output_dir: &Path) -> Result<()> {
    if cfg!(target_os = "windows") {
        let status = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    archive.display(),
                    output_dir.display()
                ),
            ])
            .status()?;
        if !status.success() {
            return Err(anyhow!("解压 zip 失败"));
        }
    } else {
        let status = Command::new("unzip")
            .arg("-o")
            .arg(archive)
            .arg("-d")
            .arg(output_dir)
            .status()?;
        if !status.success() {
            return Err(anyhow!("解压 zip 失败"));
        }
    }
    Ok(())
}

fn extract_targz_with_system(archive: &Path, output_dir: &Path) -> Result<()> {
    let status = Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(output_dir)
        .status()?;
    if !status.success() {
        return Err(anyhow!("解压 tar.gz 失败"));
    }
    Ok(())
}

fn file_name_from_url(url: &str) -> Option<String> {
    let trimmed = url.split('?').next().unwrap_or(url);
    trimmed.rsplit('/').next().map(|v| v.to_string())
}
