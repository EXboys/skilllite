//! Long text processing: truncation and chunked LLM summarization.
//!
//! Ported from Python `extensions/long_text.py`.
//!
//! Strategy: head + tail (articles: intro + conclusion are most important).
//! When tool results exceed the summarization threshold, we:
//!   1. Split into chunks of `SKILLLITE_CHUNK_SIZE` (~6000 chars)
//!   2. Select head chunks + tail chunks (skip middle)
//!   3. Summarize each chunk via LLM (≤ 500 chars per chunk)
//!   4. Combine summaries; if still too long, do a final merge pass
//!
//! Configurable via environment variables (see `types.rs` env helpers):
//!   SKILLLITE_CHUNK_SIZE, SKILLLITE_HEAD_CHUNKS, SKILLLITE_TAIL_CHUNKS,
//!   SKILLLITE_MAX_OUTPUT_CHARS, SKILLLITE_SUMMARIZE_THRESHOLD

use anyhow::Result;

use super::llm::LlmClient;
use super::types::{self, ChatMessage, safe_truncate, safe_slice_from, chunk_str};

/// Simple truncation with notice.
/// Ported from Python `truncate_content`.
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

/// Summarize long content using LLM with head+tail chunking strategy.
///
/// Ported from Python `summarize_long_content`.
///
/// Algorithm:
///   1. Divide content into chunks of `chunk_size`
///   2. If total ≤ head_size + tail_size: process all chunks
///   3. Otherwise: take first `head_chunks` + last `tail_chunks` chunks
///   4. Summarize each chunk individually (≤ 500 chars per chunk)
///   5. Combine summaries; if combined > `max_output_chars`, do final merge
///
/// Returns the summarized text, or falls back to truncation on error.
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

    // Split content into chunks and select head+tail
    let (chunks, truncated_note) = select_chunks(
        content,
        chunk_size,
        head_size,
        tail_size,
        total_len,
    );

    if chunks.is_empty() {
        return "(内容为空)".to_string();
    }

    // Summarize each chunk via LLM
    let mut chunk_summaries = Vec::new();
    for (idx, chunk) in chunks.iter().enumerate() {
        match summarize_single_chunk(client, model, chunk).await {
            Ok(summary) if !summary.is_empty() => {
                chunk_summaries.push(summary);
            }
            Ok(_) => {
                chunk_summaries.push(format!("[段 {} 总结为空]", idx + 1));
            }
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

    // If combined is within limits, return directly
    if combined.len() <= max_output_chars {
        return combined;
    }

    // Final merge pass: merge all summaries into one
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

/// Select head + tail chunks from content.
/// Returns (chunks, truncated_note).
fn select_chunks(
    content: &str,
    chunk_size: usize,
    head_size: usize,
    tail_size: usize,
    total_len: usize,
) -> (Vec<String>, String) {
    if total_len <= head_size + tail_size {
        // Content fits — process all chunks (UTF-8 safe via chunk_str)
        let chunks: Vec<String> = chunk_str(content, chunk_size)
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string())
            .collect();
        (chunks, String::new())
    } else {
        // Take head chunks + tail chunks, skip middle (UTF-8 safe)
        let mut chunks = Vec::new();

        // Head chunks: take the first `head_size` bytes (at a safe boundary)
        let head_content = safe_truncate(content, head_size);
        for chunk in chunk_str(head_content, chunk_size) {
            if !chunk.trim().is_empty() {
                chunks.push(chunk.to_string());
            }
        }

        // Tail chunks: take the last `tail_size` bytes (at a safe boundary)
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

/// Summarize a single chunk via LLM (target ≤ 500 chars).
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

/// Merge multiple chunk summaries into one concise summary (target ≤ 3000 chars).
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
