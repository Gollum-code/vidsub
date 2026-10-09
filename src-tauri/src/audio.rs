use std::path::Path;
use std::process::Command;

pub fn check_ffmpeg(ffmpeg: &str) -> bool {
    Command::new(ffmpeg)
        .arg("-version")
        .output()
        .map(|o| o.status.success() || o.status.code().is_some())
        .unwrap_or(false)
}

pub fn probe_duration(ffprobe: &str, input: &Path) -> Result<f64, String> {
    let output = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(input)
        .output()
        .map_err(|e| format!("无法运行 ffprobe: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    stdout
        .parse::<f64>()
        .map_err(|_| "无法解析视频时长，请确认文件是有效的视频/音频".to_string())
}

pub fn extract_audio(ffmpeg: &str, input: &Path, wav_path: &Path) -> Result<(), String> {
    let output = Command::new(ffmpeg)
        .arg("-y")
        .arg("-i")
        .arg(input)
        .arg("-vn")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg("-f")
        .arg("wav")
        .arg(wav_path)
        .output()
        .map_err(|e| format!("无法运行 ffmpeg: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg 提取音轨失败: {}", stderr.lines().next_back().unwrap_or("未知错误")));
    }

    if !wav_path.exists() {
        return Err("ffmpeg 未能生成音频文件".to_string());
    }
    Ok(())
}
