use std::path::PathBuf;

use vidsub_lib::{align, audio, srt, tools, whisper};

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let input = PathBuf::from(args.get(1).ok_or("usage: subpipe <media> [out_dir]")?);
    if !input.exists() {
        return Err(format!("输入文件不存在: {}", input.display()));
    }
    let out_dir = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| input.parent().map(PathBuf::from).unwrap_or_default());
    let base = input
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("无法解析文件名")?
        .to_string();

    let ffmpeg = tools::ffmpeg()
        .map(|p| tools::path_to_string(&p))
        .unwrap_or_else(|| "ffmpeg".to_string());
    let ffprobe = tools::ffprobe()
        .map(|p| tools::path_to_string(&p))
        .unwrap_or_else(|| "ffprobe".to_string());
    let whisper = tools::whisper_cli()
        .map(|p| tools::path_to_string(&p))
        .unwrap_or_else(|| "whisper-cli".to_string());
    let model = tools::model_small()
        .map(|p| tools::path_to_string(&p))
        .ok_or("未找到 ggml-small.bin，请放到 tools 目录")?;

    let duration = audio::probe_duration(&ffprobe, &input)?;
    println!("duration={:.1}s", duration);

    let wav = out_dir.join(format!("{base}_audio16k.wav"));
    audio::extract_audio(&ffmpeg, &input, &wav)?;
    println!("audio={}", wav.display());

    let prefix = out_dir.join(&base);
    let segs = whisper::transcribe(&whisper, &model, &wav, "auto", &prefix)?;
    let segs = align::align(segs, 10.0);

    let srt_path = out_dir.join(format!("{base}.srt"));
    std::fs::write(&srt_path, srt::to_srt(&segs)).map_err(|e| e.to_string())?;
    println!("srt={}", srt_path.display());
    println!("segments={}", segs.len());
    for s in segs.iter().take(5) {
        println!("[{:.1} -> {:.1}] {}", s.start, s.end, s.text.trim());
    }

    Ok(())
}