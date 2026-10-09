use crate::Segment;

pub fn to_srt(segments: &[Segment]) -> String {
    let mut out = String::new();
    for (i, seg) in segments.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            fmt_time(seg.start),
            fmt_time(seg.end),
            seg.text.trim()
        ));
    }
    out
}

fn fmt_time(sec: f64) -> String {
    let t = if sec < 0.0 { 0.0 } else { sec };
    let total_ms = (t * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}
