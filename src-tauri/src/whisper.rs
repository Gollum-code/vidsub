use crate::Segment;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[derive(Deserialize)]
struct WhisperJson {
    #[serde(default)]
    segments: Vec<WhisperSegment>,
    #[serde(default, rename = "transcription")]
    transcription: Vec<WhisperTranscription>,
}

#[derive(Deserialize)]
struct WhisperSegment {
    start: f64,
    end: f64,
    text: String,
}

#[derive(Deserialize)]
struct WhisperTranscription {
    text: String,
    #[serde(default)]
    offsets: Offsets,
}

#[derive(Deserialize, Default)]
struct Offsets {
    from: f64,
    to: f64,
}

pub fn check(whisper_path: &str) -> bool {
    Command::new(whisper_path)
        .arg("--help")
        .output()
        .is_ok()
}

pub fn transcribe(
    whisper_path: &str,
    model_path: &str,
    wav_path: &Path,
    language: &str,
    out_prefix: &Path,
) -> Result<Vec<Segment>, String> {
    let args: Vec<String> = vec![
        "-m".to_string(),
        model_path.to_string(),
        "-f".to_string(),
        wav_path.to_string_lossy().to_string(),
        "-l".to_string(),
        language.to_string(),
        "-of".to_string(),
        out_prefix.to_string_lossy().to_string(),
    ];

    let json_path = out_prefix.with_extension("json");
    let mut success = false;

    for flag in ["-ojf", "-oj"] {
        let mut try_args = args.clone();
        try_args.push(flag.to_string());
        let output = Command::new(whisper_path)
            .args(&try_args)
            .output()
            .map_err(|e| format!("无法运行 whisper-cli: {e}"))?;
        if output.status.success() && json_path.exists() {
            success = true;
            break;
        }
    }

    if !success {
        return Err("whisper-cli 运行失败，请检查可执行文件路径和模型文件".to_string());
    }

    parse_json(&json_path)
}

pub fn parse_json(path: &Path) -> Result<Vec<Segment>, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("读取识别结果失败: {e}"))?;
    let parsed: WhisperJson =
        serde_json::from_str(&raw).map_err(|e| format!("解析识别结果 JSON 失败: {e}"))?;

    let mut out: Vec<Segment> = parsed
        .segments
        .into_iter()
        .map(|s| Segment {
            start: s.start,
            end: s.end,
            text: s.text.trim().to_string(),
        })
        .collect();

    if out.is_empty() {
        out = parsed
            .transcription
            .into_iter()
            .map(|s| Segment {
                start: s.offsets.from / 1000.0,
                end: s.offsets.to / 1000.0,
                text: s.text.trim().to_string(),
            })
            .collect();
    }

    Ok(out)
}
