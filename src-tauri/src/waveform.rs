use crate::audio;
use std::path::Path;

pub struct Waveform {
    pub duration: f64,
    pub peaks: Vec<f32>,
}

pub fn extract_waveform(input: &Path, ffmpeg: &str, ffprobe: &str) -> Result<Waveform, String> {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("wave")
        .to_string();
    let tmp_dir = std::env::temp_dir().join("vidsub-wave");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("创建临时目录失败: {e}"))?;
    let wav = tmp_dir.join(format!("{stem}_{}.wav", uuid::Uuid::new_v4()));

    let duration = audio::probe_duration(ffprobe, input)?;
    audio::extract_audio(ffmpeg, input, &wav)?;

    let peaks = parse_pcm16(&wav, 1200)?;
    let _ = std::fs::remove_file(&wav);
    Ok(Waveform { duration, peaks })
}

fn parse_pcm16(path: &Path, buckets: usize) -> Result<Vec<f32>, String> {
    let raw = std::fs::read(path).map_err(|e| format!("读取波形数据失败: {e}"))?;
    if raw.len() < 44 || &raw[0..4] != b"RIFF" || &raw[8..12] != b"WAVE" {
        return Err("无效的 WAV 文件".to_string());
    }

    let mut data_offset: Option<usize> = None;
    let mut data_len: usize = 0;
    let mut sample_rate: u32 = 0;
    let mut bits: u16 = 16;
    let mut channels: u16 = 1;

    let mut pos = 12usize;
    while pos + 8 <= raw.len() {
        let id = &raw[pos..pos + 4];
        let size = le_u32(&raw[pos + 4..pos + 8]) as usize;
        let body = pos + 8;
        match id {
            b"fmt " => {
                channels = le_u16(&raw[body + 2..body + 4]);
                sample_rate = le_u32(&raw[body + 4..body + 8]);
                bits = le_u16(&raw[body + 14..body + 16]);
            }
            b"data" => {
                data_offset = Some(body);
                data_len = size;
            }
            _ => {}
        }
        pos = body + size + (size % 2);
    }

    let offset = data_offset.ok_or("WAV 缺少 data 块")?;
    let len = data_len;
    if bits != 16 {
        return Err("仅支持 16bit PCM".to_string());
    }

    let bytes_per_frame = (bits / 8) as usize * channels as usize;
    if bytes_per_frame == 0 {
        return Err("数据格式错误".to_string());
    }
    let frame_count = len.min(raw.len().saturating_sub(offset)) / bytes_per_frame;

    let mut peaks = vec![0.0f32; buckets];
    if frame_count == 0 || buckets == 0 {
        return Ok(peaks);
    }

    let per = (frame_count as f64 / buckets as f64).max(1.0);
    let mut idx = 0usize;
    let frame = 0usize;
    while frame < frame_count && idx < buckets {
        let end = ((idx + 1) as f64 * per) as usize;
        let start = (idx as f64 * per) as usize;
        let mut max_amp: i32 = 0;
        let mut f = start;
        while f < end.max(start + 1) && f < frame_count {
            if frame_count > 1 && f >= frame_count {
                break;
            }
            let p = offset + f * bytes_per_frame;
            let mut v = (raw[p] as i16) | ((raw[p + 1] as i16) << 8);
            if channels > 1 {
                let p2 = offset + f * bytes_per_frame;
                let v2 = (raw[p2] as i16) | ((raw[p2 + 1] as i16) << 8);
                v = ((v as i32 + v2 as i32) / 2) as i16;
            }
            let a = (v as i32).abs();
            if a > max_amp {
                max_amp = a;
            }
            f += 1;
        }
        peaks[idx] = (max_amp as f32) / 32768.0;
        idx += 1;
    }

    let _ = sample_rate;
    Ok(peaks)
}

fn le_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn le_u16(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_wav(path: &Path, samples: &[i16]) {
        let bytes_per_sample = 2u32;
        let data_len = (samples.len() as u32) * bytes_per_sample;
        let mut buf = Vec::new();
        buf.write_all(b"RIFF").unwrap();
        buf.write_all(&(36 + data_len).to_le_bytes()).unwrap();
        buf.write_all(b"WAVE").unwrap();
        buf.write_all(b"fmt ").unwrap();
        buf.write_all(&16u32.to_le_bytes()).unwrap();
        buf.write_all(&1u16.to_le_bytes()).unwrap();
        buf.write_all(&1u16.to_le_bytes()).unwrap();
        buf.write_all(&16000u32.to_le_bytes()).unwrap();
        buf.write_all(&(16000u32 * 2).to_le_bytes()).unwrap();
        buf.write_all(&2u16.to_le_bytes()).unwrap();
        buf.write_all(&16u16.to_le_bytes()).unwrap();
        buf.write_all(b"data").unwrap();
        buf.write_all(&data_len.to_le_bytes()).unwrap();
        for s in samples {
            buf.write_all(&s.to_le_bytes()).unwrap();
        }
        std::fs::write(path, &buf).unwrap();
    }

    #[test]
    fn parses_pcm16_peaks() {
        let mut samples = Vec::new();
        for i in 0..800 {
            samples.push((((i % 16) - 8) * 2000) as i16);
        }
        let path = std::env::temp_dir().join(format!("wave_test_{}.wav", uuid::Uuid::new_v4()));
        make_wav(&path, &samples);
        let peaks = parse_pcm16(&path, 8).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(peaks.len(), 8);
        assert!(peaks.iter().all(|p| *p >= 0.0 && *p <= 1.0));
        assert!(peaks.iter().any(|p| *p > 0.0));
    }
}
