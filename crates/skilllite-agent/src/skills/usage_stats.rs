
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use chrono::{Utc, DateTime};

const USAGE_STATS_FILE: &str = "skill_usage_stats.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SkillUsage {
    pub success_count: u64,
    pub failure_count: u64,
    pub last_used_time: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SkillUsageStats {
    stats: HashMap<String, SkillUsage>,
}

impl SkillUsageStats {
    pub fn load(data_dir: &Path) -> Result<Self> {
        let file_path = data_dir.join(USAGE_STATS_FILE);
        if !file_path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(file_path)?;
        let stats: HashMap<String, SkillUsage> = serde_json::from_str(&content)?;
        Ok(Self { stats })
    }

    pub fn save(&self, data_dir: &Path) -> Result<()> {
        fs::create_dir_all(data_dir)?;
        let file_path = data_dir.join(USAGE_STATS_FILE);
        let content = serde_json::to_string_pretty(&self.stats)?;
        fs::write(file_path, content)?;
        Ok(())
    }

    pub fn update_usage(&mut self, skill_name: &str, success: bool) {
        let entry = self.stats.entry(skill_name.to_string()).or_insert_with(|| SkillUsage {
            success_count: 0,
            failure_count: 0,
            last_used_time: Utc::now(),
        });
        if success {
            entry.success_count += 1;
        } else {
            entry.failure_count += 1;
        }
        entry.last_used_time = Utc::now();
    }
}

// Global instance for usage stats, protected by a mutex
lazy_static::lazy_static! {
    pub static ref GLOBAL_USAGE_STATS: Arc<Mutex<SkillUsageStats>> = {
        let data_dir = skilllite_core::config::PathsConfig::from_env().data_dir.clone();
        let stats = SkillUsageStats::load(&data_dir)
            .unwrap_or_else(|e| {
                eprintln!("Failed to load skill usage stats: {}", e);
                SkillUsageStats::default()
            });
        Arc::new(Mutex::new(stats))
    };
}

pub fn track_skill_execution(skill_name: &str, success: bool) {
    let mut stats = GLOBAL_USAGE_STATS
        .lock()
        .expect("global usage stats mutex poisoned");
    stats.update_usage(skill_name, success);
    let data_dir = skilllite_core::config::PathsConfig::from_env().data_dir;
    if let Err(e) = stats.save(&data_dir) {
        eprintln!("Failed to save skill usage stats: {}", e);
    }
}
