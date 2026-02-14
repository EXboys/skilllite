"""
Long text processing: truncation and chunked summarization.

Used by AgenticLoop when tool results (e.g. HTTP responses) exceed context limits.
Strategy: head + tail (articles: intro + conclusion are most important).
"""

from typing import Any, Callable, List, Optional


# Output max length (~2k tokens)
MAX_OUTPUT_CHARS = 8000

# Threshold above which we use chunked summarization instead of truncation
SUMMARIZE_THRESHOLD = 15000

# Chunk size (~1.5k tokens per chunk)
CHUNK_SIZE = 6000

# Chunks from head and tail (articles: intro + conclusion are most important)
# Total: HEAD_CHUNKS + TAIL_CHUNKS chunks, then 1 merge = 7 LLM calls max
HEAD_CHUNKS = 3
TAIL_CHUNKS = 3


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
    max_output_chars: int = MAX_OUTPUT_CHARS,
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
    total_len = len(content)
    head_size = HEAD_CHUNKS * CHUNK_SIZE
    tail_size = TAIL_CHUNKS * CHUNK_SIZE

    if total_len <= head_size + tail_size:
        chunks = []
        for i in range(0, total_len, CHUNK_SIZE):
            c = content[i : i + CHUNK_SIZE]
            if c.strip():
                chunks.append(c)
        truncated_note = ""
    else:
        head_chunks = [
            content[i : i + CHUNK_SIZE]
            for i in range(0, min(head_size, total_len), CHUNK_SIZE)
            if content[i : i + CHUNK_SIZE].strip()
        ]
        tail_chunks = [
            content[i : i + CHUNK_SIZE]
            for i in range(max(0, total_len - tail_size), total_len, CHUNK_SIZE)
            if content[i : i + CHUNK_SIZE].strip()
        ]
        chunks = head_chunks + tail_chunks
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
