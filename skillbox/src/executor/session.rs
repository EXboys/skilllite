//! Session store: sessions.json
//!
//! Schema aligned with OpenClaw: sessionId, sessionKey, token counts, compaction state.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub session_id: String,
    pub session_key: String,
    pub updated_at: String,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub context_tokens: u64,
    #[serde(default)]
    pub compaction_count: u32,
    #[serde(default)]
    pub memory_flush_at: Option<String>,
    #[serde(default)]
    pub memory_flush_compaction_count: Option<u32>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStore {
    pub sessions: HashMap<String, SessionEntry>,
}

impl Default for SessionStore {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

impl SessionStore {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read session store: {}", path.display()))?;
        serde_json::from_str(&content).with_context(|| "Invalid sessions.json")
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content).with_context(|| format!("Failed to write session store: {}", path.display()))
    }

    pub fn get(&self, session_key: &str) -> Option<&SessionEntry> {
        self.sessions.get(session_key)
    }

    pub fn create_or_get(&mut self, session_key: &str) -> &mut SessionEntry {
        let now = chrono_now();
        if !self.sessions.contains_key(session_key) {
            let entry = SessionEntry {
                session_id: format!("tx-{}", uuid_short()),
                session_key: session_key.to_string(),
                updated_at: now.clone(),
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                context_tokens: 0,
                compaction_count: 0,
                memory_flush_at: None,
                memory_flush_compaction_count: None,
                extra: HashMap::new(),
            };
            self.sessions.insert(session_key.to_string(), entry);
        }
        let entry = self.sessions.get_mut(session_key).unwrap();
        entry.updated_at = now;
        entry
    }

    pub fn update(&mut self, session_key: &str, f: impl FnOnce(&mut SessionEntry)) -> Result<()> {
        let entry = self
            .sessions
            .get_mut(session_key)
            .context("Session not found")?;
        f(entry);
        entry.updated_at = chrono_now();
        Ok(())
    }
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{}", secs)
}

fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", t % 0xFFFF_FFFF)
}
