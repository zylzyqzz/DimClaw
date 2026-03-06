use anyhow::Result;
use async_trait::async_trait;

use super::{Hand, HandResult, ScheduleConfig};

#[derive(Default)]
pub struct ClipHand;

#[async_trait]
impl Hand for ClipHand {
    fn name(&self) -> &str { "clip" }
    fn description(&self) -> &str { "视频剪辑手：调用 yt-dlp/ffmpeg/whisper 处理视频" }
    fn schedule(&self) -> ScheduleConfig {
        ScheduleConfig { cron: None, interval: Some(3600), condition: None, timezone: "Asia/Shanghai".to_string() }
    }
    async fn execute(&self) -> Result<HandResult> {
        let output = if cfg!(target_os = "windows") {
            "Clip 已就绪（请确保系统已安装 yt-dlp/ffmpeg/whisper）"
        } else {
            "Clip ready (requires yt-dlp/ffmpeg/whisper in PATH)"
        };
        Ok(HandResult::ok(output))
    }
}
