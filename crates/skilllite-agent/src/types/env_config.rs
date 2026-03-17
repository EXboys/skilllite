//! Environment config helpers for long-text summarization, tool result limits, etc.
//!
//! Ported from Python `config/env_config.py`.

/// Helper: read an env var as usize with fallback.
fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Chunk size for long text summarization (~1.5k tokens). `SKILLLITE_CHUNK_SIZE`.
pub fn get_chunk_size() -> usize {
    env_usize("SKILLLITE_CHUNK_SIZE", 6000)
}

/// Number of head chunks for head+tail summarization. `SKILLLITE_HEAD_CHUNKS`.
pub fn get_head_chunks() -> usize {
    env_usize("SKILLLITE_HEAD_CHUNKS", 3)
}

/// Number of tail chunks for head+tail summarization. `SKILLLITE_TAIL_CHUNKS`.
pub fn get_tail_chunks() -> usize {
    env_usize("SKILLLITE_TAIL_CHUNKS", 3)
}

/// Max output length for summarization (~2k tokens). `SKILLLITE_MAX_OUTPUT_CHARS`.
pub fn get_max_output_chars() -> usize {
    env_usize("SKILLLITE_MAX_OUTPUT_CHARS", 8000)
}

/// Model for Map stage in MapReduce summarization. `SKILLLITE_MAP_MODEL`.
/// When set, Map (per-chunk summarization) uses this cheaper model; Reduce (merge) uses main model.
/// E.g. `qwen-plus`, `gemini-1.5-flash`. If unset, both stages use main model.
pub fn get_map_model(main_model: &str) -> String {
    skilllite_core::config::loader::env_optional("SKILLLITE_MAP_MODEL", &[])
        .unwrap_or_else(|| main_model.to_string())
}

/// Long text selection strategy. `SKILLLITE_LONG_TEXT_STRATEGY`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongTextStrategy {
    /// Head + tail only (existing behavior).
    HeadTailOnly,
    /// Score all chunks (Position + Discourse + Entity), take top-K.
    HeadTailExtract,
    /// Map all chunks (no filtering), Reduce merge. Best with SKILLLITE_MAP_MODEL.
    MapReduceFull,
}

fn env_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn get_long_text_strategy() -> LongTextStrategy {
    let v = env_str("SKILLLITE_LONG_TEXT_STRATEGY", "head_tail_only")
        .to_lowercase()
        .trim()
        .to_string();
    match v.as_str() {
        "head_tail_extract" | "extract" => LongTextStrategy::HeadTailExtract,
        "mapreduce_full" | "mapreduce" | "map_reduce" => LongTextStrategy::MapReduceFull,
        _ => LongTextStrategy::HeadTailOnly,
    }
}

/// Number of chunks to select in extract mode. Uses ratio or head+tail count as floor.
pub fn get_extract_top_k(total_chunks: usize, head_chunks: usize, tail_chunks: usize) -> usize {
    let ratio = std::env::var("SKILLLITE_EXTRACT_TOP_K_RATIO")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.5);
    let by_ratio = (total_chunks as f64 * ratio).ceil() as usize;
    let floor = head_chunks + tail_chunks;
    by_ratio.max(floor).min(total_chunks)
}

/// Threshold above which chunked LLM summarization is used instead of simple
/// truncation. `SKILLLITE_SUMMARIZE_THRESHOLD`.
/// Default raised from 15000→30000 to avoid summarizing medium-sized HTML/code
/// files (e.g. 17KB website) which destroys content needed for downstream tasks.
pub fn get_summarize_threshold() -> usize {
    env_usize("SKILLLITE_SUMMARIZE_THRESHOLD", 30000)
}

/// Max output tokens for LLM completion. `SKILLLITE_MAX_TOKENS`.
/// Higher values reduce write_output/write_file truncation when generating large content.
/// Default 8192 to match common API limits (e.g. DeepSeek). Set higher if your API supports it.
pub fn get_max_tokens() -> usize {
    env_usize("SKILLLITE_MAX_TOKENS", 8192)
}

/// Max chars for a single user input message before truncation/summarization.
/// `SKILLLITE_USER_INPUT_MAX_CHARS`. Default 30000 (~7.5k tokens).
/// Inputs shorter than this pass through unchanged; longer inputs are
/// truncated (if ≤ `SKILLLITE_SUMMARIZE_THRESHOLD`) or LLM-summarized.
pub fn get_user_input_max_chars() -> usize {
    env_usize("SKILLLITE_USER_INPUT_MAX_CHARS", 30000)
}

/// Max chars per tool result. `SKILLLITE_TOOL_RESULT_MAX_CHARS`.
/// Default raised from 8000→12000 to better accommodate HTML/code tool results
/// without triggering unnecessary truncation.
pub fn get_tool_result_max_chars() -> usize {
    env_usize("SKILLLITE_TOOL_RESULT_MAX_CHARS", 12000)
}

/// Max chars for tool messages during context-overflow recovery.
/// `SKILLLITE_TOOL_RESULT_RECOVERY_MAX_CHARS`.
pub fn get_tool_result_recovery_max_chars() -> usize {
    env_usize("SKILLLITE_TOOL_RESULT_RECOVERY_MAX_CHARS", 3000)
}

/// Output directory override. `SKILLLITE_OUTPUT_DIR`.
pub fn get_output_dir() -> Option<String> {
    skilllite_core::config::PathsConfig::from_env().output_dir
}

/// Compaction threshold: compact conversation history when message count exceeds this.
/// `SKILLLITE_COMPACTION_THRESHOLD`. Default 16 (~8 turns).
pub fn get_compaction_threshold() -> usize {
    env_usize("SKILLLITE_COMPACTION_THRESHOLD", 16)
}

/// Whether to run pre-compaction memory flush (OpenClaw-style).
/// When enabled, before compacting we run a silent agent turn to remind the model
/// to write durable memories. `SKILLLITE_MEMORY_FLUSH_ENABLED`. Default true.
pub fn get_memory_flush_enabled() -> bool {
    let v = skilllite_core::config::loader::env_optional("SKILLLITE_MEMORY_FLUSH_ENABLED", &[]);
    match v.as_deref().map(|s| s.to_lowercase()) {
        Some(s) if matches!(s.as_str(), "0" | "false" | "no" | "off") => false,
        _ => true,
    }
}

/// Memory flush threshold: run memory flush when history approaches compaction.
/// Lower value = more frequent memory flush. Use same as compaction if not set.
/// `SKILLLITE_MEMORY_FLUSH_THRESHOLD`. Default 12 (so flush triggers ~4 msgs before compaction at 16).
pub fn get_memory_flush_threshold() -> usize {
    env_usize("SKILLLITE_MEMORY_FLUSH_THRESHOLD", 12)
}

/// Number of recent messages to keep after compaction. `SKILLLITE_COMPACTION_KEEP_RECENT`.
pub fn get_compaction_keep_recent() -> usize {
    env_usize("SKILLLITE_COMPACTION_KEEP_RECENT", 10)
}

/// Whether to use compact planning prompt (rule filtering + fewer examples).
/// - If SKILLLITE_COMPACT_PLANNING is set: use that (1=compact, 0=full).
/// - If not set: only latest/best models (claude, gpt-4, gpt-5, gemini-2) use compact; deepseek, qwen, 7b, ollama etc. get full.
pub fn get_compact_planning(model: Option<&str>) -> bool {
    if let Some(v) = skilllite_core::config::loader::env_optional(
        skilllite_core::config::env_keys::misc::SKILLLITE_COMPACT_PLANNING,
        &[],
    ) {
        return !matches!(v.to_lowercase().as_str(), "0" | "false" | "no" | "off");
    }
    // Auto: only top-tier models use compact; others (deepseek, qwen, 7b, ollama) get full prompt
    let model = match model {
        Some(m) => m.to_lowercase(),
        None => return false, // unknown model → full
    };
    let compact_models = ["claude-4.6", "gpt-4.5", "gpt-5", "gemini-2.5", "gemini-3.0"];
    compact_models
        .iter()
        .any(|p| model.starts_with(p) || model.contains(p))
}
