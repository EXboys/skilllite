//! Memory tools for the agent: search, write, list.
//!
//! Wraps the executor-layer `executor::memory` module to provide
//! agent-facing memory tools (memory_search, memory_write, memory_list).
//! Ported from Python `extensions/memory.py`.

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::json;
use std::path::Path;

use crate::agent::types::{FunctionDef, ToolDefinition, ToolResult};

// ─── Tool definitions ───────────────────────────────────────────────────────

/// Get memory tool definitions for the LLM.
pub fn get_memory_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "memory_search".to_string(),
                description: "Search the agent's memory (BM25). Use keywords in the \
                    same language as stored content. Returns relevant memory chunks \
                    ranked by relevance."
                    .to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Search query (use keywords in the same language as stored content)"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of results (default: 10)"
                        }
                    },
                    "required": ["query"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "memory_write".to_string(),
                description: "Store information in the agent's memory for future \
                    retrieval. Use this to save user preferences, conversation \
                    summaries, or any information that should persist across sessions."
                    .to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {
                        "rel_path": {
                            "type": "string",
                            "description": "Relative path within memory directory (e.g. 'preferences/theme.md')"
                        },
                        "content": {
                            "type": "string",
                            "description": "Content to store (markdown format recommended)"
                        },
                        "append": {
                            "type": "boolean",
                            "description": "If true, append to existing file instead of overwriting. Default: false."
                        }
                    },
                    "required": ["rel_path", "content"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".to_string(),
            function: FunctionDef {
                name: "memory_list".to_string(),
                description: "List all memory files stored by the agent.".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
    ]
}

/// Check if a tool name is a memory tool.
pub fn is_memory_tool(name: &str) -> bool {
    matches!(name, "memory_search" | "memory_write" | "memory_list")
}

// ─── Tool execution ─────────────────────────────────────────────────────────

/// Execute a memory tool.
/// Memory is stored in ~/.skilllite/chat/memory, not the workspace.
pub fn execute_memory_tool(
    tool_name: &str,
    arguments: &str,
    _workspace: &Path,
    agent_id: &str,
) -> ToolResult {
    let args: serde_json::Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            return ToolResult {
                tool_call_id: String::new(),
                tool_name: tool_name.to_string(),
                content: format!("Invalid arguments JSON: {}", e),
                is_error: true,
            };
        }
    };

    let mem_root = crate::executor::chat_root();
    let result = match tool_name {
        "memory_search" => execute_memory_search(&args, &mem_root, agent_id),
        "memory_write" => execute_memory_write(&args, &mem_root, agent_id),
        "memory_list" => execute_memory_list(&mem_root),
        _ => Err(anyhow::anyhow!("Unknown memory tool: {}", tool_name)),
    };

    match result {
        Ok(content) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content,
            is_error: false,
        },
        Err(e) => ToolResult {
            tool_call_id: String::new(),
            tool_name: tool_name.to_string(),
            content: format!("Error: {}", e),
            is_error: true,
        },
    }
}

/// Search memory using BM25.
fn execute_memory_search(
    args: &serde_json::Value,
    chat_root: &Path,
    agent_id: &str,
) -> Result<String> {
    let query = args
        .get("query")
        .and_then(|v| v.as_str())
        .context("'query' is required")?;
    let limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(10);

    let idx_path = crate::executor::memory::index_path(chat_root, agent_id);
    if !idx_path.exists() {
        return Ok("No memory index found. Memory is empty.".to_string());
    }

    let conn = Connection::open(&idx_path).context("Failed to open memory index")?;
    crate::executor::memory::ensure_index(&conn)?;

    let hits = crate::executor::memory::search_bm25(&conn, query, limit)?;
    if hits.is_empty() {
        return Ok(format!("No results found for query: '{}'", query));
    }

    let mut result = format!("Found {} results for '{}':\n\n", hits.len(), query);
    for (i, hit) in hits.iter().enumerate() {
        result.push_str(&format!(
            "--- Result {} (file: {}, score: {:.2}) ---\n{}\n\n",
            i + 1,
            hit.path,
            hit.score,
            hit.content
        ));
    }
    Ok(result)
}

/// Write content to memory and index it for BM25 search.
fn execute_memory_write(
    args: &serde_json::Value,
    chat_root: &Path,
    agent_id: &str,
) -> Result<String> {
    let rel_path = args
        .get("rel_path")
        .and_then(|v| v.as_str())
        .context("'rel_path' is required")?;
    let content = args
        .get("content")
        .and_then(|v| v.as_str())
        .context("'content' is required")?;
    let append = args
        .get("append")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let memory_dir = chat_root.join("memory");
    let file_path = memory_dir.join(rel_path);

    // Security: ensure path stays within memory directory
    let normalized = normalize_memory_path(&file_path);
    if !normalized.starts_with(&memory_dir) {
        anyhow::bail!("Path escapes memory directory: {}", rel_path);
    }

    // Create parent directories
    if let Some(parent) = file_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Write or append
    let final_content = if append && file_path.exists() {
        let existing = std::fs::read_to_string(&file_path).unwrap_or_default();
        format!("{}\n\n{}", existing, content)
    } else {
        content.to_string()
    };

    std::fs::write(&file_path, &final_content)
        .with_context(|| format!("Failed to write memory file: {}", file_path.display()))?;

    // Index the file for BM25 search
    let idx_path = crate::executor::memory::index_path(chat_root, agent_id);
    if let Some(parent) = idx_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&idx_path).context("Failed to open memory index")?;
    crate::executor::memory::ensure_index(&conn)?;
    crate::executor::memory::index_file(&conn, rel_path, &final_content)?;

    Ok(format!(
        "Successfully wrote {} chars to memory://{}",
        final_content.len(),
        rel_path
    ))
}

/// List all memory files.
fn execute_memory_list(chat_root: &Path) -> Result<String> {
    let memory_dir = chat_root.join("memory");
    if !memory_dir.exists() {
        return Ok("Memory directory is empty (no files stored yet).".to_string());
    }

    let mut files = Vec::new();
    collect_memory_files(&memory_dir, &memory_dir, &mut files)?;

    if files.is_empty() {
        return Ok("Memory directory exists but contains no .md files.".to_string());
    }

    let mut result = format!("Memory files ({}):\n", files.len());
    for f in &files {
        result.push_str(&format!("  - {}\n", f));
    }
    Ok(result)
}

/// Recursively collect .md files in memory directory (skip .sqlite files).
fn collect_memory_files(
    base: &Path,
    current: &Path,
    files: &mut Vec<String>,
) -> Result<()> {
    if !current.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_memory_files(base, &path, files)?;
        } else if path.extension().map_or(false, |ext| ext == "md") {
            if let Ok(rel) = path.strip_prefix(base) {
                files.push(rel.to_string_lossy().to_string());
            }
        }
    }
    Ok(())
}

// ─── Memory context for chat sessions ───────────────────────────────────────

/// Build memory context by searching for relevant memories.
/// Returns a context string to inject into the system prompt, or None if empty.
/// Ported from Python `build_memory_context`.
/// Memory is stored in ~/.skilllite/chat/memory.
pub fn build_memory_context(
    _workspace: &Path,
    agent_id: &str,
    user_message: &str,
) -> Option<String> {
    let chat_root = crate::executor::chat_root();
    let idx_path = crate::executor::memory::index_path(&chat_root, agent_id);
    if !idx_path.exists() {
        return None;
    }

    let conn = match Connection::open(&idx_path) {
        Ok(c) => c,
        Err(_) => return None,
    };
    if crate::executor::memory::ensure_index(&conn).is_err() {
        return None;
    }

    let hits = match crate::executor::memory::search_bm25(&conn, user_message, 5) {
        Ok(h) => h,
        Err(_) => return None,
    };

    if hits.is_empty() {
        return None;
    }

    let mut context = String::from("\n\n## Relevant Memory Context\n\n");
    for hit in &hits {
        let truncated: String = hit.content.chars().take(500).collect();
        context.push_str(&format!("**[{}]**: {}\n\n", hit.path, truncated));
    }

    Some(context)
}

// ─── Path helpers ───────────────────────────────────────────────────────────

/// Normalize a path by resolving `.` and `..` components without filesystem access.
fn normalize_memory_path(path: &Path) -> std::path::PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}
