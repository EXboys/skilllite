"""
Long text processing: truncation and chunked summarization.

Used by AgenticLoop when tool results (e.g. HTTP responses) exceed context limits.
Strategy: head + tail (articles: intro + conclusion are most important).

Configurable via environment variables:
  SKILLLITE_CHUNK_SIZE         - Chunk size (~1.5k tokens), default 6000
  SKILLLITE_HEAD_CHUNKS        - Head chunks count, default 3
  SKILLLITE_TAIL_CHUNKS        - Tail chunks count, default 3
  SKILLLITE_MAX_OUTPUT_CHARS   - Max output length (~2k tokens), default 8000
  SKILLLITE_SUMMARIZE_THRESHOLD - Threshold for summarization vs truncation, default 15000
"""

from typing import Any, Callable, List, Optional

from ..config.env_config import (
    get_long_text_chunk_size,
    get_long_text_head_chunks,
    get_long_text_tail_chunks,
    get_long_text_max_output_chars,
    get_long_text_summarize_threshold,
)


# Legacy module-level constants (for backward compat). Use getters for fresh env values.
def _chunk_size() -> int:
    return get_long_text_chunk_size()


def _head_chunks() -> int:
    return get_long_text_head_chunks()


def _tail_chunks() -> int:
    return get_long_text_tail_chunks()


# Exported for backward compat (env-aware at import time)
SUMMARIZE_THRESHOLD = get_long_text_summarize_threshold()
MAX_OUTPUT_CHARS = get_long_text_max_output_chars()


def truncate_content(content: str, max_chars: int) -> str:
    """Truncate content with truncation notice."""
    if len(content) <= max_chars:
        return content
    return content[:max_chars] + f"\n\n[... 结果已截断，原文共 {len(content)} 字符，仅保留前 {max_chars} 字符 ...]"


def summarize_long_content(
    client: Any,
    model: str,
    content: str,
    api_format: str = "openai",
    max_output_chars: Optional[int] = None,
    logger: Optional[Callable[[str], None]] = None,
) -> str:
    """
    Chunk long content: take head + tail (most important for articles),
    summarize each chunk, then merge into final summary.

    Args:
        client: LLM client (OpenAI-compatible or Anthropic)
        model: Model name
        content: Long text to summarize
        api_format: "openai" or "claude_native"
        max_output_chars: Max length of output
        logger: Optional callback for log messages

    Returns:
        Summarized text
    """
    if max_output_chars is None:
        max_output_chars = get_long_text_max_output_chars()
    chunk_size = _chunk_size()
    head_chunks = _head_chunks()
    tail_chunks = _tail_chunks()
    head_size = head_chunks * chunk_size
    tail_size = tail_chunks * chunk_size
    total_len = len(content)

    if total_len <= head_size + tail_size:
        chunks = []
        for i in range(0, total_len, chunk_size):
            c = content[i : i + chunk_size]
            if c.strip():
                chunks.append(c)
        truncated_note = ""
    else:
        head_chunk_list = [
            content[i : i + chunk_size]
            for i in range(0, min(head_size, total_len), chunk_size)
            if content[i : i + chunk_size].strip()
        ]
        tail_chunk_list = [
            content[i : i + chunk_size]
            for i in range(max(0, total_len - tail_size), total_len, chunk_size)
            if content[i : i + chunk_size].strip()
        ]
        chunks = head_chunk_list + tail_chunk_list
        truncated_note = f"\n\n[注：原文 {total_len} 字符，仅总结开头与结尾]"

    if not chunks:
        return "(内容为空)"

    prompt_chunk = """Summarize the key information from this text excerpt. Keep it concise (under 500 chars).
Focus on: rankings, statistics, facts, dates, names, key findings. Preserve numbers.
Output in the same language as the input. Output summary only, no preamble."""

    chunk_summaries: List[str] = []
    for idx, chunk in enumerate(chunks):
        try:
            if api_format == "claude_native":
                resp = client.messages.create(
                    model=model,
                    max_tokens=512,
                    messages=[{"role": "user", "content": f"{prompt_chunk}\n\n---\n{chunk}"}],
                )
                summary = ""
                for block in resp.content:
                    if hasattr(block, "text"):
                        summary += block.text
            else:
                resp = client.chat.completions.create(
                    model=model,
                    messages=[{"role": "user", "content": f"{prompt_chunk}\n\n---\n{chunk}"}],
                    max_tokens=512,
                )
                summary = (resp.choices[0].message.content or "").strip()
            if summary:
                chunk_summaries.append(summary)
        except Exception as e:
            if logger:
                logger(f"[Summarize] Chunk {idx + 1} failed: {e}")
            chunk_summaries.append(f"[段 {idx + 1} 总结失败]")

    combined = "\n\n".join(chunk_summaries) + truncated_note
    if len(combined) <= max_output_chars:
        return combined

    # Final pass: merge the combined summaries
    try:
        final_prompt = """The following are summaries of different parts of a long document. 
Merge them into one concise summary (under 3000 chars). Preserve all key facts, numbers, rankings.
Output in the same language. Output summary only."""

        if api_format == "claude_native":
            resp = client.messages.create(
                model=model,
                max_tokens=1024,
                messages=[{"role": "user", "content": f"{final_prompt}\n\n---\n{combined}"}],
            )
            out = ""
            for block in resp.content:
                if hasattr(block, "text"):
                    out += block.text
        else:
            resp = client.chat.completions.create(
                model=model,
                messages=[{"role": "user", "content": f"{final_prompt}\n\n---\n{combined}"}],
                max_tokens=1024,
            )
            out = (resp.choices[0].message.content or "").strip()
        return (out or combined[:max_output_chars]) + truncated_note
    except Exception as e:
        if logger:
            logger(f"[Summarize] Final merge failed: {e}")
        return combined[:max_output_chars] + truncated_note + "\n\n[... 总结后仍过长，已截断 ...]"
