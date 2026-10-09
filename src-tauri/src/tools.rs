use std::path::{Path, PathBuf};

fn candidate(rel: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join("tools").join(rel));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let root3 = dir.join("..").join("..").join("..");
            let root4 = dir.join("..").join("..").join("..").join("..");
            out.push(root3.join("tools").join(rel));
            out.push(root4.join("tools").join(rel));
            out.push(dir.join("resources").join(rel));
            out.push(dir.join("..").join("resources").join(rel));
        }
    }
    out
}

fn find(rel: &str) -> Option<PathBuf> {
    candidate(rel)
        .into_iter()
        .find(|p| p.is_file())
}

pub fn ffmpeg() -> Option<PathBuf> {
    find("bin/ffmpeg.exe")
}

pub fn ffprobe() -> Option<PathBuf> {
    find("bin/ffprobe.exe")
}

pub fn whisper_cli() -> Option<PathBuf> {
    find("bin/whisper-cli.exe")
}

pub fn model_small() -> Option<PathBuf> {
    find("ggml-small.bin")
}

pub fn path_to_string(p: &Path) -> String {
    p.to_string_lossy().to_string()
}