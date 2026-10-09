use crate::Segment;

pub fn align(segments: Vec<Segment>, max_len: f64) -> Vec<Segment> {
    let mut out = Vec::new();
    for seg in segments {
        if seg.text.trim().is_empty() {
            continue;
        }
        split_range(seg.text.trim(), seg.start, seg.end, max_len, &mut out);
    }
    out
}

fn split_range(text: &str, start: f64, end: f64, max_len: f64, out: &mut Vec<Segment>) {
    let duration = end - start;
    if duration <= max_len || text.chars().count() <= 1 {
        out.push(Segment {
            start,
            end,
            text: text.trim().to_string(),
        });
        return;
    }

    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    let mid = total / 2;
    let window_lo = (total as f64 * 0.3) as usize;
    let window_hi = (total as f64 * 0.7) as usize;
    let breaks = ['。', '！', '？', '！', '？', '.', '!', '?', '…', '，', ',', '；', ';', '：', ':', '、'];

    let mut cut = None;
    let mut best_dist = usize::MAX;
    for (i, c) in chars.iter().enumerate() {
        if i < window_lo || i > window_hi || !breaks.contains(c) {
            continue;
        }
        let dist = i.abs_diff(mid);
        if dist < best_dist {
            best_dist = dist;
            cut = Some(i + 1);
        }
    }
    if cut.is_none() {
        cut = Some(mid);
    }

    let cut = cut.unwrap();
    let left: String = chars[..cut].iter().collect();
    let right: String = chars[cut..].iter().collect();
    if left.trim().is_empty() || right.trim().is_empty() {
        out.push(Segment {
            start,
            end,
            text: text.trim().to_string(),
        });
        return;
    }

    let frac = cut as f64 / total as f64;
    let mid_time = start + duration * frac;
    split_range(left.trim(), start, mid_time, max_len, out);
    split_range(right.trim(), mid_time, end, max_len, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_long_segment_at_punctuation() {
        let seg = Segment {
            start: 0.0,
            end: 20.0,
            text: "第一句。第二句。第三句。第四句。第五句。".to_string(),
        };
        let out = align(vec![seg], 6.0);
        assert!(out.len() >= 3, "expected multiple chunks, got {}", out.len());
        let mut prev_end = 0.0;
        for s in &out {
            assert!(s.start >= prev_end - 1e-9);
            assert!(s.end > s.start);
            prev_end = s.end;
        }
    }

    #[test]
    fn keeps_short_segment() {
        let seg = Segment {
            start: 1.0,
            end: 3.0,
            text: "短句".to_string(),
        };
        let out = align(vec![seg], 10.0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "短句");
    }

    #[test]
    fn drops_empty_segments() {
        let out = align(
            vec![Segment {
                start: 0.0,
                end: 1.0,
                text: "  ".to_string(),
            }],
            10.0,
        );
        assert!(out.is_empty());
    }
}