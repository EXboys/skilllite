//! A3: LLM admission scan result cache.
//!
//! Persists scan results to ~/.skilllite/scan-cache.json. Key = content_hash (SHA256 of
//! skill_md + script_samples). Same hash within TTL avoids redundant LLM calls.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const CACHE_TTL_SECS: u64 = 300;
const CACHE_FILENAME: &str = "scan-cache.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedEntry {
    risk: String,
    reason: String,
    timestamp: u64,
}

fn cache_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".skilllite")
        .join(CACHE_FILENAME)
}

/// Compute SHA256 hash of content for cache key.
pub fn content_hash(skill_md: &str, script_samples: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(skill_md.as_bytes());
    hasher.update(script_samples.as_bytes());
    hex::encode(hasher.finalize())
}

/// Look up cached LLM admission result. Returns (risk, reason) if found and not expired.
pub fn get_cached(content_hash: &str) -> Result<Option<(String, String)>> {
    let path = cache_path();
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path).map_err(|e| anyhow::anyhow!("read cache: {}", e))?;
    let map: HashMap<String, CachedEntry> =
        serde_json::from_str(&content).unwrap_or_default();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Some(entry) = map.get(content_hash) {
        if now.saturating_sub(entry.timestamp) < CACHE_TTL_SECS {
            return Ok(Some((entry.risk.clone(), entry.reason.clone())));
        }
    }
    Ok(None)
}

/// Store LLM admission result in cache.
pub fn put_cached(content_hash: &str, risk: &str, reason: &str) -> Result<()> {
    let path = cache_path();
    let parent = path.parent().unwrap_or(path.as_path());
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut map: HashMap<String, CachedEntry> = if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        HashMap::new()
    };
    // Evict expired entries before adding
    map.retain(|_, e| now.saturating_sub(e.timestamp) < CACHE_TTL_SECS);
    map.insert(
        content_hash.to_string(),
        CachedEntry {
            risk: risk.to_string(),
            reason: reason.to_string(),
            timestamp: now,
        },
    );
    let content = serde_json::to_string_pretty(&map)?;
    fs::write(&path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_hash_deterministic() {
        let h1 = content_hash("skill a", "script b");
        let h2 = content_hash("skill a", "script b");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA256 hex
    }

    #[test]
    fn test_content_hash_different_inputs() {
        let h1 = content_hash("a", "b");
        let h2 = content_hash("a", "c");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_cache_roundtrip() {
        let hash = "test_hash_123";
        put_cached(hash, "suspicious", "test reason").unwrap();
        let cached = get_cached(hash).unwrap();
        assert!(cached.is_some());
        let (risk, reason) = cached.unwrap();
        assert_eq!(risk, "suspicious");
        assert_eq!(reason, "test reason");
    }
}
