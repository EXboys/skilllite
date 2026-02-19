//! Long text processing: truncation and chunked selection/summarization.
//!
//! Strategy: head + tail (default), or head_tail_extract (Position + Discourse + Entity scoring).
//! Configurable via `SKILLLITE_LONG_TEXT_STRATEGY`:
//!   - `head_tail_only`: take first N + last M chunks (existing behavior)
//!   - `head_tail_extract`: score all chunks, take top-K by score, preserve order
//!
//! Env: SKILLLITE_CHUNK_SIZE, SKILLLITE_HEAD_CHUNKS, SKILLLITE_TAIL_CHUNKS,
//!      SKILLLITE_MAX_OUTPUT_CHARS, SKILLLITE_LONG_TEXT_STRATEGY, SKILLLITE_EXTRACT_TOP_K_RATIO

use anyhow::Result;

use super::llm::LlmClient;
use super::types::{self, ChatMessage, chunk_str, safe_slice_from, safe_truncate};

mod filter;

/// Simple truncation with notice.
pub fn truncate_content(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }
    format!(
        "{}\n\n[... 结果已截断，原文共 {} 字符，仅保留前 {} 字符 ...]",
        safe_truncate(content, max_chars),
        content.len(),
        max_chars
    )
}

/// Summarize long content using LLM with configurable chunk selection strategy.
pub async fn summarize_long_content(
    client: &LlmClient,
    model: &str,
    content: &str,
) -> String {
    let chunk_size = types::get_chunk_size();
    let head_chunks_count = types::get_head_chunks();
    let tail_chunks_count = types::get_tail_chunks();
    let max_output_chars = types::get_max_output_chars();

    let total_len = content.len();
    let head_size = head_chunks_count * chunk_size;
    let tail_size = tail_chunks_count * chunk_size;

    let (chunks, truncated_note) = select_chunks(
        content,
        chunk_size,
        head_size,
        tail_size,
        total_len,
        head_chunks_count,
        tail_chunks_count,
    );

    if chunks.is_empty() {
        return "(内容为空)".to_string();
    }

    let mut chunk_summaries = Vec::new();
    for (idx, chunk) in chunks.iter().enumerate() {
        match summarize_single_chunk(client, model, chunk).await {
            Ok(summary) if !summary.is_empty() => chunk_summaries.push(summary),
            Ok(_) => chunk_summaries.push(format!("[段 {} 总结为空]", idx + 1)),
            Err(e) => {
                tracing::warn!("Chunk {} summarization failed: {}", idx + 1, e);
                chunk_summaries.push(format!("[段 {} 总结失败]", idx + 1));
            }
        }
    }

    let combined = if truncated_note.is_empty() {
        chunk_summaries.join("\n\n")
    } else {
        format!("{}{}", chunk_summaries.join("\n\n"), truncated_note)
    };

    if combined.len() <= max_output_chars {
        return combined;
    }

    match merge_summaries(client, model, &combined).await {
        Ok(merged) => {
            let result = if merged.is_empty() {
                truncate_content(&combined, max_output_chars)
            } else {
                merged
            };
            if truncated_note.is_empty() {
                result
            } else {
                format!("{}{}", result, truncated_note)
            }
        }
        Err(e) => {
            tracing::warn!("Final merge failed: {}", e);
            format!(
                "{}{}\n\n[... 总结后仍过长，已截断 ...]",
                truncate_content(&combined, max_output_chars),
                truncated_note
            )
        }
    }
}

/// Select chunks: head+tail only, or scored extract (Position + Discourse + Entity).
fn select_chunks(
    content: &str,
    chunk_size: usize,
    head_size: usize,
    tail_size: usize,
    total_len: usize,
    head_chunks_count: usize,
    tail_chunks_count: usize,
) -> (Vec<String>, String) {
    let strategy = types::get_long_text_strategy();
    let all_chunks: Vec<String> = chunk_str(content, chunk_size)
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .collect();

    if all_chunks.is_empty() {
        return (Vec::new(), String::new());
    }

    match strategy {
        types::LongTextStrategy::HeadTailOnly => {
            select_head_tail_only(
                content,
                &all_chunks,
                chunk_size,
                head_size,
                tail_size,
                total_len,
                head_chunks_count,
                tail_chunks_count,
            )
        }
        types::LongTextStrategy::HeadTailExtract => {
            select_by_score(
                &all_chunks,
                total_len,
                head_chunks_count,
                tail_chunks_count,
            )
        }
    }
}

fn select_head_tail_only(
    content: &str,
    all_chunks: &[String],
    chunk_size: usize,
    head_size: usize,
    tail_size: usize,
    total_len: usize,
    _head_chunks_count: usize,
    _tail_chunks_count: usize,
) -> (Vec<String>, String) {
    if total_len <= head_size + tail_size {
        (all_chunks.to_vec(), String::new())
    } else {
        let mut chunks = Vec::new();
        let head_content = safe_truncate(content, head_size);
        for chunk in chunk_str(head_content, chunk_size) {
            if !chunk.trim().is_empty() {
                chunks.push(chunk.to_string());
            }
        }
        let tail_content = safe_slice_from(content, total_len.saturating_sub(tail_size));
        for chunk in chunk_str(tail_content, chunk_size) {
            if !chunk.trim().is_empty() {
                chunks.push(chunk.to_string());
            }
        }
        let note = format!("\n\n[注：原文 {} 字符，仅总结开头与结尾]", total_len);
        (chunks, note)
    }
}

fn select_by_score(
    all_chunks: &[String],
    total_len: usize,
    head_chunks_count: usize,
    tail_chunks_count: usize,
) -> (Vec<String>, String) {
    let total_chunks = all_chunks.len();
    let top_k = types::get_extract_top_k(total_chunks, head_chunks_count, tail_chunks_count);

    let mut scored: Vec<(usize, String, f64)> = all_chunks
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.clone(), filter::score_chunk(c, i, total_chunks)))
        .collect();

    scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let selected: Vec<(usize, String)> = scored
        .into_iter()
        .take(top_k)
        .map(|(i, c, _)| (i, c))
        .collect();
    let mut ordered: Vec<(usize, String)> = selected;
    ordered.sort_by_key(|(i, _)| *i);

    let chunks: Vec<String> = ordered.into_iter().map(|(_, c)| c).collect();
    let note = format!(
        "\n\n[注：原文 {} 字符，共 {} 段，按信息量选取 {} 段]",
        total_len, total_chunks, top_k
    );
    (chunks, note)
}

async fn summarize_single_chunk(
    client: &LlmClient,
    model: &str,
    chunk: &str,
) -> Result<String> {
    let prompt = format!(
        "Summarize the key information from this text excerpt. Keep it concise (under 500 chars).\n\
         Focus on: rankings, statistics, facts, dates, names, key findings. Preserve numbers.\n\
         Output in the same language as the input. Output summary only, no preamble.\n\n\
         ---\n{}",
        chunk
    );

    let messages = vec![ChatMessage::user(&prompt)];
    let resp = client
        .chat_completion(model, &messages, None, Some(0.3))
        .await?;

    let text = resp
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default()
        .trim()
        .to_string();

    Ok(text)
}

async fn merge_summaries(
    client: &LlmClient,
    model: &str,
    combined: &str,
) -> Result<String> {
    let prompt = format!(
        "The following are summaries of different parts of a long document.\n\
         Merge them into one concise summary (under 3000 chars). Preserve all key facts, numbers, rankings.\n\
         Output in the same language. Output summary only.\n\n\
         ---\n{}",
        combined
    );

    let messages = vec![ChatMessage::user(&prompt)];
    let resp = client
        .chat_completion(model, &messages, None, Some(0.3))
        .await?;

    let text = resp
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default()
        .trim()
        .to_string();

    Ok(text)
}
