//! Memory store: MEMORY.md, memory/*.md + SQLite FTS5 (BM25).
//! With `memory_vector` feature: sqlite-vec for semantic search.

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

const CHUNK_TOKEN_TARGET: usize = 400;
const CHUNK_OVERLAP: usize = 80;

#[cfg(feature = "memory_vector")]
use std::sync::Once;

#[cfg(feature = "memory_vector")]
static VEC_INIT: Once = Once::new();

/// Load sqlite-vec extension. Must be called before opening any connection that uses vec0.
#[cfg(feature = "memory_vector")]
fn ensure_vec_extension_loaded() {
    VEC_INIT.call_once(|| {
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

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

/// Ensure vec0 table exists for vector search. Call after ensure_index.
/// If dimension changed (e.g. switched from Qwen 1024 to OpenAI 1536), drops old table
/// and recreates with new dimension. Vec index will be empty until memory_write repopulates.
#[cfg(feature = "memory_vector")]
pub fn ensure_vec0_table(conn: &Connection, dimension: usize) -> Result<()> {
    ensure_vec_extension_loaded();

    // Create metadata table to track dimension
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS _memory_vec_meta (
            k TEXT PRIMARY KEY,
            v INTEGER NOT NULL
        );
        "#,
    )?;

    let stored_dim: Option<i64> = conn
        .query_row(
            "SELECT v FROM _memory_vec_meta WHERE k = 'dimension'",
            [],
            |row| row.get(0),
        )
        .ok();

    let need_recreate = match stored_dim {
        Some(d) if d as usize == dimension => false,
        _ => true,
    };

    if need_recreate {
        conn.execute_batch("DROP TABLE IF EXISTS memory_vec")?;
        conn.execute(
            "INSERT OR REPLACE INTO _memory_vec_meta (k, v) VALUES ('dimension', ?)",
            rusqlite::params![dimension as i64],
        )?;

        let sql = format!(
            r#"CREATE VIRTUAL TABLE memory_vec USING vec0(
                embedding float[{}],
                path text,
                chunk_index int,
                +content text
            )"#,
            dimension
        );
        conn.execute_batch(&sql)?;
        tracing::info!(
            dimension,
            "memory_vec table recreated for new embedding dimension"
        );
    } else {
        // Table may not exist yet (first run)
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='memory_vec'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|n| n > 0)
            .unwrap_or(false);

        if !exists {
            conn.execute(
                "INSERT OR REPLACE INTO _memory_vec_meta (k, v) VALUES ('dimension', ?)",
                rusqlite::params![dimension as i64],
            )?;
            let sql = format!(
                r#"CREATE VIRTUAL TABLE memory_vec USING vec0(
                    embedding float[{}],
                    path text,
                    chunk_index int,
                    +content text
                )"#,
                dimension
            );
            conn.execute_batch(&sql)?;
        }
    }

    Ok(())
}

/// Chunk markdown content by paragraphs, target ~400 tokens per chunk.
#[cfg(feature = "memory_vector")]
pub fn chunk_content_for_embed(content: &str) -> Vec<String> {
    chunk_content(content)
}

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
/// Removes existing chunks for this path before re-indexing (handles overwrite).
pub fn index_file(conn: &Connection, path: &str, content: &str) -> Result<()> {
    conn.execute("DELETE FROM memory_fts WHERE path = ?", rusqlite::params![path])?;
    let chunks = chunk_content(content);
    for (i, chunk) in chunks.iter().enumerate() {
        conn.execute(
            "INSERT INTO memory_fts(path, chunk_index, content) VALUES (?, ?, ?)",
            rusqlite::params![path, i as i64, chunk],
        )?;
    }
    Ok(())
}

/// Index chunks with embeddings into vec0. Removes existing rows for this path first.
#[cfg(feature = "memory_vector")]
pub fn index_file_vec(
    conn: &Connection,
    path: &str,
    chunks: &[String],
    embeddings: &[Vec<f32>],
) -> Result<()> {
    use zerocopy::AsBytes;
    if chunks.len() != embeddings.len() {
        anyhow::bail!(
            "Chunks and embeddings length mismatch: {} vs {}",
            chunks.len(),
            embeddings.len()
        );
    }
    conn.execute("DELETE FROM memory_vec WHERE path = ?", rusqlite::params![path])?;
    let mut stmt = conn.prepare(
        "INSERT INTO memory_vec(path, chunk_index, content, embedding) VALUES (?, ?, ?, ?)",
    )?;
    for (i, (chunk, emb)) in chunks.iter().zip(embeddings.iter()).enumerate() {
        stmt.execute(rusqlite::params![path, i as i64, chunk, emb.as_bytes()])?;
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

/// Search using vector similarity (vec0 KNN).
#[cfg(feature = "memory_vector")]
pub fn search_vec(
    conn: &Connection,
    query_embedding: &[f32],
    limit: i64,
) -> Result<Vec<MemoryHit>> {
    use zerocopy::AsBytes;
    let mut stmt = conn.prepare(
        r#"
        SELECT path, chunk_index, content, distance
        FROM memory_vec
        WHERE embedding MATCH ?1
        ORDER BY distance
        LIMIT ?2
        "#,
    )?;
    let rows = stmt.query_map(
        rusqlite::params![query_embedding.as_bytes(), limit],
        |row| {
            Ok(MemoryHit {
                path: row.get(0)?,
                chunk_index: row.get(1)?,
                content: row.get(2)?,
                // vec0 returns distance (lower = more similar). Negate for "score" semantics.
                score: -row.get::<_, f64>(3).unwrap_or(0.0),
            })
        },
    )?;
    Ok(rows.filter_map(|r| r.ok()).collect())
}

/// Check if vec0 table has any rows (vector index is populated).
#[cfg(feature = "memory_vector")]
pub fn has_vec_index(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT COUNT(*) FROM memory_vec",
        [],
        |row| row.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .unwrap_or(false)
}

#[derive(Debug, Clone)]
pub struct MemoryHit {
    pub path: String,
    pub chunk_index: i64,
    pub content: String,
    pub score: f64,
}
