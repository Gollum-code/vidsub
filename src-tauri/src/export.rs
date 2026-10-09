use crate::Segment;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SubFormat {
    Srt,
    Vtt,
    Ass,
}

impl SubFormat {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "srt" => Ok(SubFormat::Srt),
            "vtt" => Ok(SubFormat::Vtt),
            "ass" => Ok(SubFormat::Ass),
            _ => Err("不支持的导出格式".to_string()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AssStyle {
    pub font: String,
    pub size: i32,
    pub primary: String,
    pub outline_color: String,
    pub back_color: String,
    pub bold: bool,
    pub outline: i32,
    pub shadow: i32,
    pub margin_v: i32,
}

pub fn styles() -> Vec<(String, AssStyle)> {
    vec![
        (
            "白字黑边（默认）".to_string(),
            AssStyle {
                font: "Microsoft YaHei".to_string(),
                size: 48,
                primary: "#FFFFFF".to_string(),
                outline_color: "#000000".to_string(),
                back_color: "#000000".to_string(),
                bold: true,
                outline: 2,
                shadow: 1,
                margin_v: 36,
            },
        ),
        (
            "黄字黑边".to_string(),
            AssStyle {
                font: "Microsoft YaHei".to_string(),
                size: 48,
                primary: "#FFD700".to_string(),
                outline_color: "#000000".to_string(),
                back_color: "#000000".to_string(),
                bold: true,
                outline: 2,
                shadow: 1,
                margin_v: 36,
            },
        ),
        (
            "白字无描边".to_string(),
            AssStyle {
                font: "Microsoft YaHei".to_string(),
                size: 44,
                primary: "#FFFFFF".to_string(),
                outline_color: "#000000".to_string(),
                back_color: "#000000".to_string(),
                bold: false,
                outline: 0,
                shadow: 0,
                margin_v: 24,
            },
        ),
        (
            "大字号".to_string(),
            AssStyle {
                font: "Microsoft YaHei".to_string(),
                size: 64,
                primary: "#FFFFFF".to_string(),
                outline_color: "#000000".to_string(),
                back_color: "#000000".to_string(),
                bold: true,
                outline: 3,
                shadow: 2,
                margin_v: 40,
            },
        ),
    ]
}

pub fn get_style(index: usize) -> AssStyle {
    styles()
        .into_iter()
        .nth(index)
        .map(|(_, s)| s)
        .unwrap_or_else(|| styles()[0].1.clone())
}

pub fn render(format: SubFormat, segments: &[Segment], style: &AssStyle) -> String {
    match format {
        SubFormat::Srt => to_srt(segments),
        SubFormat::Vtt => to_vtt(segments),
        SubFormat::Ass => to_ass(segments, style),
    }
}

pub fn to_srt(segments: &[Segment]) -> String {
    let mut out = String::new();
    for (i, seg) in segments.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            fmt_srt_time(seg.start),
            fmt_srt_time(seg.end),
            seg.text.trim()
        ));
    }
    out
}

fn fmt_srt_time(sec: f64) -> String {
    let t = sec.max(0.0);
    let total_ms = (t * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

pub fn to_vtt(segments: &[Segment]) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for seg in segments {
        out.push_str(&format!(
            "{} --> {}\n{}\n\n",
            fmt_vtt_time(seg.start),
            fmt_vtt_time(seg.end),
            seg.text.trim()
        ));
    }
    out
}

fn fmt_vtt_time(sec: f64) -> String {
    let t = sec.max(0.0);
    let total_ms = (t * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{:02}:{:02}:{:02}.{:03}", h, m, s, ms)
}

pub fn to_ass(segments: &[Segment], style: &AssStyle) -> String {
    let mut out = String::new();
    out.push_str("[Script Info]\n");
    out.push_str("ScriptType: v4.00+\n");
    out.push_str("PlayResX: 1920\n");
    out.push_str("PlayResY: 1080\n");
    out.push_str("WrapStyle: 0\n");
    out.push_str("ScaledBorderAndShadow: yes\n\n");

    out.push_str("[V4+ Styles]\n");
    out.push_str("Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n");
    let bold = if style.bold { "-1" } else { "0" };
    out.push_str(&format!(
        "Style: Default,{},{},{},{},{},{},{},0,0,0,100,100,0,0,1,{},{},2,20,20,{},1\n\n",
        style.font,
        style.size,
        ass_color(&style.primary),
        ass_color(&style.primary),
        ass_color(&style.outline_color),
        ass_color(&style.back_color),
        bold,
        style.outline,
        style.shadow,
        style.margin_v,
    ));

    out.push_str("[Events]\n");
    out.push_str("Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");
    for seg in segments {
        out.push_str(&format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
            fmt_ass_time(seg.start),
            fmt_ass_time(seg.end),
            ass_escape(&seg.text)
        ));
    }
    out
}

fn fmt_ass_time(sec: f64) -> String {
    let t = sec.max(0.0);
    let total_cs = (t * 100.0).round() as u64;
    let h = total_cs / 360_000;
    let m = (total_cs % 360_000) / 6_000;
    let s = (total_cs % 6_000) / 100;
    let cs = total_cs % 100;
    format!("{}:{:02}:{:02}.{:02}", h, m, s, cs)
}

fn ass_color(hex: &str) -> String {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(255);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(255);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(255);
    format!("&H00{:02X}{:02X}{:02X}", b, g, r)
}

fn ass_escape(text: &str) -> String {
    text.replace('{', "\\{")
        .replace('}', "\\}")
        .replace('\n', "\\N")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Segment> {
        vec![Segment {
            start: 0.0,
            end: 2.5,
            text: "你好, world".to_string(),
        }]
    }

    #[test]
    fn renders_srt() {
        let out = render(SubFormat::Srt, &sample(), &get_style(0));
        assert!(out.contains("00:00:00,000 --> 00:00:02,500"));
        assert!(out.contains("你好, world"));
    }

    #[test]
    fn renders_vtt() {
        let out = render(SubFormat::Vtt, &sample(), &get_style(0));
        assert!(out.starts_with("WEBVTT"));
        assert!(out.contains("00:00:00.000 --> 00:00:02.500"));
    }

    #[test]
    fn renders_ass() {
        let out = render(SubFormat::Ass, &sample(), &get_style(1));
        assert!(out.contains("[Events]"));
        assert!(out.contains("Dialogue: 0,0:00:00.00,0:00:02.50,Default"));
        assert!(out.contains("Style: Default,Microsoft YaHei,48"));
    }
}