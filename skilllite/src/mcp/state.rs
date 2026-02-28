//! MCP Server state: cache structures and server instance.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use skilllite_sandbox::security::types::ScanResult;

/// Cached scan result with TTL.
pub(super) struct CachedScan {
    pub scan_result: ScanResult,
    pub code_hash: String,
    #[allow(dead_code)]
    pub language: String,
    #[allow(dead_code)]
    pub code: String,
    pub created_at: Instant,
}

/// Session-level confirmation cache: skill_name → code_hash.
/// Avoids re-scanning the same skill if its code hasn't changed.
pub(super) struct ConfirmedSkill {
    pub code_hash: String,
}

/// MCP Server state maintained across requests.
pub(super) struct McpServer {
    /// Skills directory path
    pub skills_dir: PathBuf,
    /// Scan result cache: scan_id → CachedScan (TTL: 300s)
    pub scan_cache: HashMap<String, CachedScan>,
    /// Session-level confirmation cache: skill_name → ConfirmedSkill
    pub confirmed_skills: HashMap<String, ConfirmedSkill>,
    /// Scan cache TTL
    pub cache_ttl: Duration,
}

impl McpServer {
    pub fn new(skills_dir: PathBuf) -> Self {
        Self {
            skills_dir,
            scan_cache: HashMap::new(),
            confirmed_skills: HashMap::new(),
            cache_ttl: Duration::from_secs(300),
        }
    }

    /// Remove expired scan cache entries.
    pub fn cleanup_expired_scans(&mut self) {
        let now = Instant::now();
        self.scan_cache.retain(|_, v| now.duration_since(v.created_at) < self.cache_ttl);
    }

    /// Generate a code hash: SHA256(language:code) full hexdigest.
    pub fn generate_code_hash(language: &str, code: &str) -> String {
        let content = format!("{}:{}", language, code);
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Generate a scan_id: SHA256(code_hash:timestamp)[:16].
    pub fn generate_scan_id(code_hash: &str) -> String {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64()
            .to_string();
        let content = format!("{}:{}", code_hash, timestamp);
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        hex::encode(hasher.finalize())[..16].to_string()
    }

    /// Compute a hash of a skill's entry point code for confirmation cache.
    pub fn compute_skill_hash(skill_dir: &Path, entry_point: &str) -> String {
        let mut hasher = Sha256::new();
        let entry_path = if !entry_point.is_empty() {
            skill_dir.join(entry_point)
        } else {
            skill_dir.join("SKILL.md")
        };
        if let Ok(content) = std::fs::read(&entry_path) {
            hasher.update(&content);
        }
        if let Ok(skill_md) = std::fs::read(skill_dir.join("SKILL.md")) {
            hasher.update(&skill_md);
        }
        hex::encode(hasher.finalize())[..16].to_string()
    }
}

