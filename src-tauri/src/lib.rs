pub mod align;
pub mod audio;
mod export;
pub mod license;
pub mod srt;
mod translate;
mod waveform;
pub mod tools;
pub mod whisper;

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscribeResult {
    pub srt_path: String,
    pub segments: Vec<Segment>,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub whisper_path: Option<String>,
    pub model_path: Option<String>,
    pub language: Option<String>,
    pub api_provider: Option<String>,
    pub api_model: Option<String>,
    pub target_lang: Option<String>,
}

#[derive(Default)]
struct AppState {
    settings: Mutex<Settings>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DependencyCheck {
    pub ffmpeg_ok: bool,
    pub whisper_ok: bool,
    pub model_ok: bool,
    pub model_path: Option<String>,
    pub whisper_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WaveformData {
    pub duration: f64,
    pub peaks: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    pub stage: String,
    pub percent: u8,
    pub message: String,
}

fn emit_progress(app: &AppHandle, stage: &str, percent: u8, message: &str) {
    let _ = app.emit(
        "vidsub-progress",
        ProgressPayload {
            stage: stage.to_string(),
            percent,
            message: message.to_string(),
        },
    );
}

fn resolve_whisper_path(settings: &Settings) -> String {
    settings
        .whisper_path
        .clone()
        .or_else(|| std::env::var("VIDSUB_WHISPER").ok())
        .or_else(|| tools::whisper_cli().map(|p| tools::path_to_string(&p)))
        .unwrap_or_else(|| "whisper-cli".to_string())
}

fn models_cache_dir(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("models"))
}

pub const MODEL_SIZES: [&str; 3] = ["small", "base", "tiny"];

fn model_file(size: &str) -> String {
    match size {
        "base" => "ggml-base.bin".to_string(),
        "tiny" => "ggml-tiny.bin".to_string(),
        _ => "ggml-small.bin".to_string(),
    }
}

fn model_url(size: &str) -> String {
    let file = model_file(size);
    format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{file}")
}

fn cached_model(app: &AppHandle) -> Option<std::path::PathBuf> {
    let dir = models_cache_dir(app)?;
    MODEL_SIZES
        .iter()
        .map(|s| dir.join(model_file(s)))
        .find(|p| p.is_file())
}

fn resolve_model_path(settings: &Settings, app: &AppHandle) -> Option<String> {
    settings
        .model_path
        .clone()
        .or_else(|| std::env::var("VIDSUB_MODEL").ok())
        .or_else(|| cached_model(app).map(|p| tools::path_to_string(&p)))
        .or_else(|| tools::model_small().map(|p| tools::path_to_string(&p)))
}

fn resolve_ffmpeg_path() -> String {
    tools::ffmpeg()
        .map(|p| tools::path_to_string(&p))
        .unwrap_or_else(|| "ffmpeg".to_string())
}

fn resolve_ffprobe_path() -> String {
    tools::ffprobe()
        .map(|p| tools::path_to_string(&p))
        .unwrap_or_else(|| "ffprobe".to_string())
}

fn settings_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_config_dir()
        .ok()
        .map(|d| d.join("settings.json"))
}

fn save_settings(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = settings_path(app).ok_or("无法定位应用配置目录")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| format!("写入设置失败: {e}"))
}

fn load_settings(app: &AppHandle) -> Settings {
    if let Some(path) = settings_path(app) {
        if path.exists() {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if let Ok(s) = serde_json::from_str::<Settings>(&raw) {
                    return s;
                }
            }
        }
    }
    Settings::default()
}

#[tauri::command]
fn get_settings(_app: AppHandle, state: State<'_, AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn set_settings(app: AppHandle, state: State<'_, AppState>, settings: Settings) {
    let mut s = state.settings.lock().unwrap();
    if settings.whisper_path.is_some() {
        s.whisper_path = settings.whisper_path;
    }
    if settings.model_path.is_some() {
        s.model_path = settings.model_path;
    }
    if settings.language.is_some() {
        s.language = settings.language;
    }
    if settings.api_provider.is_some() {
        s.api_provider = settings.api_provider;
    }
    if settings.api_model.is_some() {
        s.api_model = settings.api_model;
    }
    if settings.target_lang.is_some() {
        s.target_lang = settings.target_lang;
    }
    let _ = save_settings(&app, &s);
}

#[tauri::command]
fn check_dependencies(
    app: AppHandle,
    state: State<'_, AppState>,
) -> DependencyCheck {
    let settings = state.settings.lock().unwrap().clone();
    let whisper_path = resolve_whisper_path(&settings);
    let model_path = resolve_model_path(&settings, &app);
    let ffmpeg_path = resolve_ffmpeg_path();
    let ffmpeg_ok = audio::check_ffmpeg(&ffmpeg_path);
    let whisper_ok = whisper::check(&whisper_path);
    let model_ok = model_path
        .as_ref()
        .map(|p| Path::new(p).exists())
        .unwrap_or(false);
    DependencyCheck {
        ffmpeg_ok,
        whisper_ok,
        model_ok,
        model_path,
        whisper_path,
    }
}

#[tauri::command]
fn get_license(app: AppHandle) -> license::LicenseInfo {
    license::current(&app)
}

#[tauri::command]
fn activate_license(app: AppHandle, key: String) -> Result<license::LicenseInfo, String> {
    license::activate(&app, &key)
}

#[tauri::command]
fn deactivate_license(app: AppHandle) -> Result<(), String> {
    license::deactivate(&app)
}

#[tauri::command]
fn get_device_id(app: AppHandle) -> String {
    license::device_id(&app)
}

#[tauri::command]
async fn download_model(app: AppHandle, size: Option<String>) -> Result<String, String> {
    if let Some(p) = cached_model(&app) {
        return Ok(tools::path_to_string(&p));
    }
    let dir = models_cache_dir(&app).ok_or("无法定位应用配置目录")?;
    let size = size.unwrap_or_else(|| "small".to_string());
    let file = model_file(&size);
    let url = model_url(&size);
    let target = dir.join(&file);
    let part = dir.join(format!("{file}.part"));

    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建模型目录失败: {e}"))?;
        let size_label = match size.as_str() {
            "tiny" => "约 75MB，速度快但准确度一般",
            "base" => "约 142MB，速度与准确度均衡",
            _ => "约 465MB，准确度最好",
        };
        emit_progress(
            &app,
            "下载模型",
            2,
            &format!("开始下载 {file}（{size_label}），请耐心等待..."),
        );
        let resp = ureq::get(&url)
            .call()
            .map_err(|e| format!("模型下载失败: {e}"))?;
        let total: u64 = resp
            .header("Content-Length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let mut reader = std::io::BufReader::new(resp.into_reader());
        let mut file = std::fs::File::create(&part).map_err(|e| format!("创建下载文件失败: {e}"))?;
        let mut buf = [0u8; 262144];
        let mut written: u64 = 0;
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("读取下载数据失败: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .map_err(|e| format!("写入下载数据失败: {e}"))?;
            written += n as u64;
            if total > 0 {
                let pct = written
                    .checked_mul(100)
                    .and_then(|v| v.checked_div(total))
                    .unwrap_or(0)
                    .min(98) as u8;
                emit_progress(
                    &app,
                    "下载模型",
                    pct,
                    &format!(
                        "下载中 {:.0}/{:.0} MB",
                        written as f64 / 1048576.0,
                        total as f64 / 1048576.0
                    ),
                );
            }
        }
        file.flush().map_err(|e| format!("刷新下载文件失败: {e}"))?;
        drop(file);
        std::fs::rename(&part, &target).map_err(|e| format!("保存模型失败: {e}"))?;
        emit_progress(&app, "下载模型", 100, "模型下载完成");
        Ok(tools::path_to_string(&target))
    })
    .await
    .map_err(|e| format!("下载任务失败: {e}"))?
}

#[tauri::command]
fn export_subtitles(
    _app: AppHandle,
    segments: Vec<Segment>,
    path: String,
    format: String,
    style_index: usize,
) -> Result<String, String> {
    let fmt = export::SubFormat::from_str(&format)?;
    let style = export::get_style(style_index);
    let text = export::render(fmt, &segments, &style);
    std::fs::write(&path, text).map_err(|e| format!("写入字幕文件失败: {e}"))?;
    Ok(path)
}

#[tauri::command]
fn get_export_styles() -> Vec<String> {
    export::styles().into_iter().map(|(n, _)| n).collect()
}

#[tauri::command]
fn burn_subtitles(
    app: AppHandle,
    input_path: String,
    out_path: String,
    segments: Vec<Segment>,
    style_index: usize,
) -> Result<String, String> {
    let ffmpeg_path = resolve_ffmpeg_path();
    if !audio::check_ffmpeg(&ffmpeg_path) {
        return Err("未找到 ffmpeg".to_string());
    }

    let out = std::path::PathBuf::from(&out_path);
    let dir = out
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建输出目录失败: {e}"))?;

    let ass_path = dir.join("_vidsub_burn.ass");
    let style = export::get_style(style_index);
    std::fs::write(&ass_path, export::to_ass(&segments, &style))
        .map_err(|e| format!("生成烧录字幕失败: {e}"))?;

    emit_progress(&app, "烧录字幕", 30, "正在烧录字幕到视频...");
    let status = std::process::Command::new(&ffmpeg_path)
        .current_dir(&dir)
        .arg("-y")
        .arg("-i")
        .arg(&input_path)
        .arg("-vf")
        .arg("ass=filename=_vidsub_burn.ass")
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-crf")
        .arg("20")
        .arg("-c:a")
        .arg("aac")
        .arg("-movflags")
        .arg("+faststart")
        .arg(&out_path)
        .output()
        .map_err(|e| format!("无法运行 ffmpeg: {e}"))?;

    let _ = std::fs::remove_file(&ass_path);
    if !status.status.success() {
        let stderr = String::from_utf8_lossy(&status.stderr);
        return Err(format!(
            "烧录失败: {}",
            stderr.lines().next_back().unwrap_or("未知错误")
        ));
    }
    emit_progress(&app, "烧录字幕", 100, "烧录完成");
    Ok(out_path)
}

#[tauri::command]
fn waveform(input_path: String) -> Result<WaveformData, String> {
    let ffmpeg = resolve_ffmpeg_path();
    let ffprobe = resolve_ffprobe_path();
    let w = waveform::extract_waveform(std::path::Path::new(&input_path), &ffmpeg, &ffprobe)?;
    Ok(WaveformData {
        duration: w.duration,
        peaks: w.peaks,
    })
}

#[tauri::command]
fn set_api_key(key: String) -> Result<(), String> {
    license::set_api_key(&key)
}

#[tauri::command]
fn get_api_key_status() -> bool {
    license::get_api_key().is_some()
}

#[tauri::command]
fn clear_api_key() -> Result<(), String> {
    license::clear_api_key()
}

#[tauri::command]
async fn translate_segments(
    _app: AppHandle,
    segments: Vec<Segment>,
    target_lang: String,
    provider: Option<String>,
) -> Result<Vec<String>, String> {
    let api_key = license::get_api_key().ok_or("请先设置 API Key（自带 Key 付费，隐私不出机器）")?;
    let provider = provider.unwrap_or_else(|| "deepseek".to_string());
    let lines: Vec<String> = segments.iter().map(|s| s.text.trim().to_string()).collect();
    tauri::async_runtime::spawn_blocking(move || {
        translate::translate_lines(&provider, &api_key, &lines, &target_lang)
    })
    .await
    .map_err(|e| format!("翻译任务失败: {e}"))?
}

#[tauri::command]
async fn transcribe(
    app: AppHandle,
    state: State<'_, AppState>,
    input_path: String,
    out_dir: Option<String>,
    model_path: Option<String>,
    whisper_path: Option<String>,
    language: Option<String>,
) -> Result<TranscribeResult, String> {
    let settings = {
        let mut s = state.settings.lock().unwrap();
        if model_path.is_some() {
            s.model_path = model_path;
        }
        if whisper_path.is_some() {
            s.whisper_path = whisper_path;
        }
        if language.is_some() {
            s.language = language;
        }
        s.clone()
    };

    let input = std::path::PathBuf::from(&input_path);
    if !input.exists() {
        return Err(format!("输入文件不存在: {input_path}"));
    }

    let out_dir = out_dir
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            input
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        });
    let base_name = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("subtitle")
        .to_string();

    tauri::async_runtime::spawn_blocking(move || {
        run_pipeline(&app, &input, &out_dir, &base_name, &settings)
    })
    .await
    .map_err(|e| format!("任务执行失败: {e}"))?
}

fn run_pipeline(
    app: &AppHandle,
    input: &Path,
    out_dir: &Path,
    base_name: &str,
    settings: &Settings,
) -> Result<TranscribeResult, String> {
    let ffmpeg_path = resolve_ffmpeg_path();

    let whisper_path = resolve_whisper_path(settings);
    let model_path = resolve_model_path(settings, app).ok_or("未配置 whisper 模型路径")?;
    if !Path::new(&model_path).exists() {
        return Err(format!("模型文件不存在: {model_path}"));
    }
    if !whisper::check(&whisper_path) {
        return Err(format!("找不到 whisper-cli: {whisper_path}"));
    }
    if !audio::check_ffmpeg(&ffmpeg_path) {
        return Err("未找到 ffmpeg".to_string());
    }

    std::fs::create_dir_all(out_dir).map_err(|e| format!("创建输出目录失败: {e}"))?;

    let wav_path = out_dir.join(format!("{base_name}_audio16k.wav"));
    emit_progress(app, "提取音轨", 10, "正在从媒体中提取音频...");
    audio::extract_audio(&ffmpeg_path, input, &wav_path)?;

    let lang = settings
        .language
        .clone()
        .unwrap_or_else(|| "auto".to_string());
    emit_progress(app, "语音识别", 30, "本地 Whisper 识别中，请稍候...");
    let out_prefix = out_dir.join(base_name);
    let mut segments = whisper::transcribe(&whisper_path, &model_path, &wav_path, &lang, &out_prefix)?;

    emit_progress(app, "字幕对齐", 85, "正在合并与分割字幕片段...");
    segments = align::align(segments, 10.0);

    let srt_path = out_dir.join(format!("{base_name}.srt"));
    let srt_text = srt::to_srt(&segments);
    std::fs::write(&srt_path, srt_text).map_err(|e| format!("写入 SRT 失败: {e}"))?;

    emit_progress(app, "完成", 100, "字幕生成完成");
    Ok(TranscribeResult {
        srt_path: srt_path.to_string_lossy().to_string(),
        segments,
        language: lang,
    })
}

pub fn run() {
    let state = AppState {
        settings: Mutex::new(Settings::default()),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(|app| {
            let loaded = load_settings(app.handle());
            let binding = app.state::<AppState>();
            let mut guard = binding.settings.lock().unwrap();
            *guard = loaded;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            set_settings,
            check_dependencies,
            get_license,
            activate_license,
            deactivate_license,
            get_device_id,
            download_model,
            export_subtitles,
            get_export_styles,
            burn_subtitles,
            waveform,
            set_api_key,
            get_api_key_status,
            clear_api_key,
            translate_segments,
            transcribe
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}