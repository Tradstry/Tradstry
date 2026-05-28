use anyhow::{Result, ensure};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfStream>,
    #[serde(default)]
    format: FfFormat,
}

#[derive(Debug, Deserialize)]
struct FfStream {
    width: Option<i64>,
    height: Option<i64>,
}

#[derive(Debug, Default, Deserialize)]
struct FfFormat {
    duration: Option<String>,
}

/// Probed video metadata.
#[derive(Debug, Clone, Copy)]
pub struct VideoMetadata {
    pub width: i64,
    pub height: i64,
    pub duration_seconds: f64,
}

/// Best-effort video probe via `ffprobe` (requires ffmpeg installed).
///
/// Returns zeroed metadata if ffprobe is unavailable or fails, so an upload is
/// never blocked by a missing/odd codec — we just store what we could read.
pub async fn probe_video(bytes: &[u8]) -> VideoMetadata {
    match probe_video_inner(bytes).await {
        Ok(meta) => meta,
        Err(error) => {
            log::warn!("ffprobe video probe failed: {error}");
            VideoMetadata {
                width: 0,
                height: 0,
                duration_seconds: 0.0,
            }
        }
    }
}

async fn probe_video_inner(bytes: &[u8]) -> Result<VideoMetadata> {
    // ffprobe needs a seekable input for reliable duration, so write a temp file.
    let tmp = std::env::temp_dir().join(format!("tradstry-probe-{}", uuid::Uuid::new_v4()));
    tokio::fs::write(&tmp, bytes).await?;

    let result = run_ffprobe(&tmp).await;
    let _ = tokio::fs::remove_file(&tmp).await;
    let output = result?;

    let width = output.streams.iter().find_map(|s| s.width).unwrap_or(0);
    let height = output.streams.iter().find_map(|s| s.height).unwrap_or(0);
    let duration_seconds = output
        .format
        .duration
        .as_deref()
        .and_then(|d| d.parse::<f64>().ok())
        .unwrap_or(0.0);

    Ok(VideoMetadata {
        width,
        height,
        duration_seconds,
    })
}

async fn run_ffprobe(path: &std::path::Path) -> Result<FfprobeOutput> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-show_entries",
            "format=duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .await?;

    ensure!(
        output.status.success(),
        "ffprobe exited with status {}",
        output.status
    );

    let parsed: FfprobeOutput = serde_json::from_slice(&output.stdout)?;
    Ok(parsed)
}
