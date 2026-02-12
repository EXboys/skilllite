//! Memory store: MEMORY.md, memory/*.md + SQLite FTS5 (BM25).
//! Vector search can be added via memory_vector feature later.

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

const CHUNK_TOKEN_TARGET: usize = 400;
const CHUNK_OVERLAP: usize = 80;

/// Get path to memory SQLite index for a workspace.
pub fn index_path(workspace_root: &Path, agent_id: &str) -> std::path::PathBuf {
    workspace_root.join("memory").join(format!("{}.sqlite", agent_id))
}

/// Ensure memory index exists with FTS5 table.
pub fn ensure_index(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE VIRTUAL TABLE IF NOT EXISTS memory_fts USING fts5(
            path,
            chunk_index,
            content,
            tokenize='porter'
        );
        "#,
    )?;
    Ok(())
}

/// Chunk markdown content by paragraphs, target ~400 tokens per chunk.
fn chunk_content(content: &str) -> Vec<String> {
    let paragraphs: Vec<&str> = content.split("\n\n").filter(|s| !s.trim().is_empty()).collect();
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut token_approx = 0;

    for p in paragraphs {
        let p_tokens = p.len() / 4; // rough token estimate
        if token_approx + p_tokens > CHUNK_TOKEN_TARGET && !current.is_empty() {
            chunks.push(current.trim().to_string());
            // overlap: keep last N tokens
            let words: Vec<&str> = current.split_whitespace().collect();
            let overlap_start = words.len().saturating_sub(CHUNK_OVERLAP / 4);
            current = words[overlap_start..].join(" ");
            token_approx = current.len() / 4;
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(p);
        token_approx += p_tokens;
    }
    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }
    chunks
}

/// Index a memory file into the SQLite DB (BM25).
pub fn index_file(conn: &Connection, path: &str, content: &str) -> Result<()> {
    let chunks = chunk_content(content);
    for (i, chunk) in chunks.iter().enumerate() {
        conn.execute(
            "INSERT INTO memory_fts(path, chunk_index, content) VALUES (?, ?, ?)",
            rusqlite::params![path, i as i64, chunk],
        )?;
    }
    Ok(())
}

/// Search using BM25 (FTS5).
pub fn search_bm25(conn: &Connection, query: &str, limit: i64) -> Result<Vec<MemoryHit>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT path, chunk_index, content, bm25(memory_fts) as rank
        FROM memory_fts
        WHERE memory_fts MATCH ?
        ORDER BY rank
        LIMIT ?
        "#,
    )?;
    let rows = stmt.query_map(rusqlite::params![query, limit], |row| {
        Ok(MemoryHit {
            path: row.get(0)?,
            chunk_index: row.get(1)?,
            content: row.get(2)?,
            score: row.get::<_, f64>(3).unwrap_or(0.0),
        })
    })?;
    let mut hits: Vec<MemoryHit> = rows.filter_map(|r| r.ok()).collect();
    hits.sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(hits)
}

#[derive(Debug, Clone)]
pub struct MemoryHit {
    pub path: String,
    pub chunk_index: i64,
    pub content: String,
    pub score: f64,
}
