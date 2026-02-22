//! Chunk scoring for extractive long-text selection.
//!
//! Lightweight, rule-based scoring: Position + DiscourseMarkers + EntityDensity.
//! No word segmentation, no external NLP crates.

use regex::Regex;
use std::sync::OnceLock;

/// Position weight: head 20% and tail 20% of chunks get 1.0, middle gets 0.25.
fn position_score(chunk_index: usize, total_chunks: usize) -> f64 {
    if total_chunks <= 1 {
        return 1.0;
    }
    let head_threshold = (total_chunks as f64 * 0.2).ceil().max(1.0) as usize;
    let tail_start = total_chunks.saturating_sub(
        (total_chunks as f64 * 0.2).ceil().max(1.0) as usize,
    );
    if chunk_index < head_threshold {
        1.0
    } else if chunk_index >= tail_start {
        1.0
    } else {
        0.25
    }
}

/// Discourse markers: sentences with summary/conclusion cues get a bonus.
/// Matches: 总结, 结论, 关键, 重要, 要点, 综上所述, 实验表明, 结果表明, 主要发现
static DISCOURSE_RE: OnceLock<Regex> = OnceLock::new();

fn discourse_score(chunk: &str) -> f64 {
    let re = DISCOURSE_RE.get_or_init(|| {
        Regex::new(r"总结|结论|关键|重要|要点|综上所述|实验表明|结果表明|主要发现|核心|概括")
            .expect("discourse regex")
    });
    let matches = re.find_iter(chunk).count();
    if matches == 0 {
        0.0
    } else {
        // Cap bonus at 1.0 (2+ matches = max)
        (matches as f64 * 0.5).min(1.0)
    }
}

/// Entity density: chunks with numbers and proper-noun-like patterns get a bonus.
static NUMBER_RE: OnceLock<Regex> = OnceLock::new();

fn entity_score(chunk: &str) -> f64 {
    let num_re = NUMBER_RE.get_or_init(|| {
        Regex::new(r"\d+[%.,]?|\d+\.\d+|[①-⑳]|[一二三四五六七八九十百千]+")
            .expect("number regex")
    });
    let num_count = num_re.find_iter(chunk).count();
    // Numbers: 0-2 = 0, 3-5 = 0.3, 6+ = 0.6
    let num_score: f64 = match num_count {
        0..=2 => 0.0,
        3..=5 => 0.3,
        _ => 0.6,
    };
    // Consecutive caps (English acronyms/names)
    let caps_count = chunk.matches(|c: char| c.is_uppercase()).count();
    let caps_score: f64 = if caps_count >= 3 { 0.2 } else { 0.0 };
    (num_score + caps_score).min(1.0)
}

/// Combined score for a chunk. Weights are tuned for balance.
/// Position is dominant (0.5), Discourse (0.3), Entity (0.2).
pub fn score_chunk(chunk: &str, chunk_index: usize, total_chunks: usize) -> f64 {
    let pos = position_score(chunk_index, total_chunks);
    let disc = discourse_score(chunk);
    let ent = entity_score(chunk);
    // Weighted sum
    0.5 * pos + 0.3 * disc + 0.2 * ent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_head_tail_high() {
        assert!(position_score(0, 10) >= 0.99);
        assert!(position_score(1, 10) >= 0.99);
        assert!(position_score(9, 10) >= 0.99);
        assert!(position_score(8, 10) >= 0.99);
    }

    #[test]
    fn discourse_markers() {
        assert!(discourse_score("这是一段普通文字") < 0.1);
        assert!(discourse_score("综上所述，我们可以得出以下结论") > 0.5);
        assert!(discourse_score("关键发现：实验表明结果显著") > 0.5);
    }

    #[test]
    fn entity_numbers() {
        assert!(entity_score("没有数字的段落") < 0.1);
        assert!(entity_score("2024年增长15.3%，达到100万") > 0.2);
    }
}
