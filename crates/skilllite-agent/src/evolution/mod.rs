//! Evolution engine: self-evolving data layer for SkillLite.
//!
//! EVO-1: Feedback collection + evaluation system + structured memory.
//! EVO-2: Prompt externalization + seed data mechanism.
//! EVO-3: Evolution engine core + evolution prompt design.
//! EVO-5: Polish + transparency (audit, degradation, CLI, time trends).
//!
//! This module manages the `decisions`, `decision_rules`, `evolution_log`,
//! and `evolution_metrics` tables in `memory/default.sqlite`, and the
//! externalized prompt/rules data in `prompts/`.

pub mod external_learner;
pub mod feedback;
pub mod prompt_learner;
pub mod seed;
pub mod skill_synth;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use rusqlite::{params, Connection};

use super::types::SkillAction;

// ─── EVO-5: Evolution mode (SKILLLITE_EVOLUTION env var) ─────────────────────

/// Which dimensions of evolution are enabled.
#[derive(Debug, Clone, PartialEq)]
pub enum EvolutionMode {
    /// All dimensions enabled (default, SKILLLITE_EVOLUTION=1 or unset).
    All,
    /// Only prompts evolution (SKILLLITE_EVOLUTION=prompts).
    PromptsOnly,
    /// Only memory evolution (SKILLLITE_EVOLUTION=memory).
    MemoryOnly,
    /// Only skills evolution (SKILLLITE_EVOLUTION=skills).
    SkillsOnly,
    /// Evolution disabled (SKILLLITE_EVOLUTION=0).
    /// Existing evolved products remain in effect (frozen), but no new evolution runs.
    Disabled,
}

impl EvolutionMode {
    /// Parse from the SKILLLITE_EVOLUTION environment variable.
    pub fn from_env() -> Self {
        match std::env::var("SKILLLITE_EVOLUTION").ok().as_deref() {
            None | Some("1") | Some("true") | Some("") => Self::All,
            Some("0") | Some("false") => Self::Disabled,
            Some("prompts") => Self::PromptsOnly,
            Some("memory") => Self::MemoryOnly,
            Some("skills") => Self::SkillsOnly,
            Some(other) => {
                tracing::warn!(
                    "Unknown SKILLLITE_EVOLUTION value '{}', defaulting to all",
                    other
                );
                Self::All
            }
        }
    }

    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }

    pub fn prompts_enabled(&self) -> bool {
        matches!(self, Self::All | Self::PromptsOnly)
    }

    pub fn memory_enabled(&self) -> bool {
        matches!(self, Self::All | Self::MemoryOnly)
    }

    pub fn skills_enabled(&self) -> bool {
        matches!(self, Self::All | Self::SkillsOnly)
    }
}

// ─── Concurrency: evolution mutex ────────────────────────────────────────────

static EVOLUTION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Try to acquire the evolution mutex. Returns true if acquired.
pub fn try_start_evolution() -> bool {
    EVOLUTION_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

/// Release the evolution mutex.
pub fn finish_evolution() {
    EVOLUTION_IN_PROGRESS.store(false, Ordering::SeqCst);
}

// ─── Atomic file writes ─────────────────────────────────────────────────────

/// Write content to a file atomically: write to .tmp, then rename.
/// POSIX rename is atomic within the same filesystem.
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ─── Evolution scope (should_evolve decision) ────────────────────────────────

/// Scope of what should be evolved in this cycle.
#[derive(Debug, Default)]
pub struct EvolutionScope {
    pub skills: bool,
    pub skill_action: SkillAction,
    pub memory: bool,
    pub prompts: bool,
    /// IDs of unprocessed decisions to be consumed.
    pub decision_ids: Vec<i64>,
}

/// Determine what should be evolved based on unprocessed decisions.
///
/// Triggers differ by dimension — Skills most aggressively, Memory moderately,
/// Prompts most conservatively (signals are noisier).
///
/// EVO-5: Respects `SKILLLITE_EVOLUTION` env var for granular control.
pub fn should_evolve(conn: &Connection) -> Result<EvolutionScope> {
    should_evolve_with_mode(conn, EvolutionMode::from_env())
}

/// Internal: `should_evolve` with an explicit mode (avoids env var races in tests).
pub fn should_evolve_with_mode(conn: &Connection, mode: EvolutionMode) -> Result<EvolutionScope> {
    if mode.is_disabled() {
        tracing::debug!("Evolution disabled via SKILLLITE_EVOLUTION=0");
        return Ok(EvolutionScope::default());
    }

    // Check daily evolution cap
    let today_evolutions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evolution_log WHERE date(ts) = date('now')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let max_per_day: i64 = std::env::var("SKILLLITE_MAX_EVOLUTIONS_PER_DAY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    if today_evolutions >= max_per_day {
        tracing::debug!("Daily evolution cap reached ({}/{})", today_evolutions, max_per_day);
        return Ok(EvolutionScope::default());
    }

    // Check minimum interval (1 hour since last evolution)
    let last_evo_hours: f64 = conn
        .query_row(
            "SELECT COALESCE(
                (julianday('now') - julianday(MAX(ts))) * 24,
                999.0
            ) FROM evolution_log",
            [],
            |row| row.get(0),
        )
        .unwrap_or(999.0);
    if last_evo_hours < 1.0 {
        tracing::debug!("Evolution too recent ({:.1}h ago, need 1h)", last_evo_hours);
        return Ok(EvolutionScope::default());
    }

    let (meaningful, failures, replans): (i64, i64, i64) = conn.query_row(
        "SELECT
            COUNT(CASE WHEN total_tools >= 2 THEN 1 END),
            COUNT(CASE WHEN failed_tools > 0 THEN 1 END),
            COUNT(CASE WHEN replans > 0 THEN 1 END)
         FROM decisions WHERE evolved = 0",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    // Collect unprocessed decision IDs
    let mut stmt = conn.prepare("SELECT id FROM decisions WHERE evolved = 0")?;
    let ids: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // Repeated pattern detection for Skill generation
    let repeated_patterns: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM (
                SELECT task_description, COUNT(*) as cnt,
                       SUM(CASE WHEN task_completed = 1 THEN 1 ELSE 0 END) as successes
                FROM decisions
                WHERE evolved = 0 AND task_description IS NOT NULL
                GROUP BY task_description
                HAVING cnt >= 3 AND CAST(successes AS REAL) / cnt >= 0.8
            )",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut scope = EvolutionScope {
        decision_ids: ids,
        ..Default::default()
    };

    // Skills: most aggressive trigger (binary feedback, cleanest signal)
    if mode.skills_enabled() && meaningful >= 3 && (failures > 0 || repeated_patterns > 0) {
        scope.skills = true;
        scope.skill_action = if repeated_patterns > 0 {
            SkillAction::Generate
        } else {
            SkillAction::Refine
        };
    }

    // Memory: moderate trigger (structured data, low risk)
    if mode.memory_enabled() && meaningful >= 3 {
        scope.memory = true;
    }

    // Prompts: most conservative trigger (noisy feedback, need more samples)
    if mode.prompts_enabled() && meaningful >= 5 && (failures >= 2 || replans >= 2) {
        scope.prompts = true;
    }

    Ok(scope)
}

// ─── 5-Layer Gatekeeper (L1-L3 for EVO-3; L4-L5 for EVO-4) ─────────────────

/// Allowed write paths for evolution (L1: path whitelist).
const ALLOWED_EVOLUTION_PATHS: &[&str] = &["prompts", "memory", "skills/_evolved"];

/// L1: Check that a path is within allowed evolution directories.
pub fn gatekeeper_l1_path(chat_root: &Path, target: &Path) -> bool {
    for allowed in ALLOWED_EVOLUTION_PATHS {
        let allowed_dir = chat_root.join(allowed);
        if target.starts_with(&allowed_dir) {
            return true;
        }
    }
    false
}

/// L1b: For template files (.md in prompts/), validate that new content preserves
/// all required placeholders. Rejects writes that would break placeholder substitution.
///
/// This is the key safety mechanism: evolution CAN improve templates, but CAN'T
/// break the placeholder contract. Data files (rules.json, examples.json) skip this.
pub fn gatekeeper_l1_template_integrity(filename: &str, new_content: &str) -> Result<()> {
    let missing = seed::validate_template(filename, new_content);
    if !missing.is_empty() {
        anyhow::bail!(
            "Gatekeeper L1b: evolved template '{}' is missing required placeholders {:?} — write rejected",
            filename,
            missing
        );
    }
    Ok(())
}

/// L2: Check that evolution change size is within limits.
/// Single evolution: rules ≤ 5, examples ≤ 3, skills ≤ 1.
pub fn gatekeeper_l2_size(new_rules: usize, new_examples: usize, new_skills: usize) -> bool {
    new_rules <= 5 && new_examples <= 3 && new_skills <= 1
}

/// Sensitive patterns for L3 content scanning.
const SENSITIVE_PATTERNS: &[&str] = &[
    "api_key", "api-key", "apikey",
    "secret", "password", "passwd",
    "token", "bearer",
    "private_key", "private-key",
    "-----BEGIN", "-----END",
    "skip scan", "bypass", "disable security",
    "eval(", "exec(", "__import__",
];

/// L3: Scan content for sensitive information (API keys, passwords, PII, bypass instructions).
pub fn gatekeeper_l3_content(content: &str) -> Result<()> {
    let lower = content.to_lowercase();
    for pattern in SENSITIVE_PATTERNS {
        if lower.contains(pattern) {
            anyhow::bail!(
                "Gatekeeper L3: evolution product contains sensitive pattern: '{}'",
                pattern
            );
        }
    }
    Ok(())
}

// ─── Version management: snapshots ──────────────────────────────────────────

fn versions_dir(chat_root: &Path) -> PathBuf {
    chat_root.join("prompts").join("_versions")
}

/// Create a pre-evolution snapshot of the given files.
/// Stores them in `_versions/{txn_id}/`.
pub fn create_snapshot(chat_root: &Path, txn_id: &str, files: &[&str]) -> Result<Vec<String>> {
    let snap_dir = versions_dir(chat_root).join(txn_id);
    std::fs::create_dir_all(&snap_dir)?;
    let prompts = chat_root.join("prompts");
    let mut backed_up = Vec::new();
    for name in files {
        let src = prompts.join(name);
        if src.exists() {
            let dst = snap_dir.join(name);
            std::fs::copy(&src, &dst)?;
            backed_up.push(name.to_string());
        }
    }
    // Prune old snapshots (keep most recent 10)
    prune_snapshots(chat_root, 10);
    Ok(backed_up)
}

/// Restore files from a snapshot identified by txn_id.
pub fn restore_snapshot(chat_root: &Path, txn_id: &str) -> Result<()> {
    let snap_dir = versions_dir(chat_root).join(txn_id);
    if !snap_dir.exists() {
        anyhow::bail!("Snapshot not found: {}", txn_id);
    }
    let prompts = chat_root.join("prompts");
    for entry in std::fs::read_dir(&snap_dir)? {
        let entry = entry?;
        let dst = prompts.join(entry.file_name());
        std::fs::copy(entry.path(), &dst)?;
    }
    tracing::info!("Restored snapshot {}", txn_id);
    Ok(())
}

fn prune_snapshots(chat_root: &Path, keep: usize) {
    let vdir = versions_dir(chat_root);
    if !vdir.exists() {
        return;
    }
    let mut dirs: Vec<_> = std::fs::read_dir(&vdir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    if dirs.len() <= keep {
        return;
    }
    dirs.sort_by_key(|e| e.file_name());
    let to_remove = dirs.len() - keep;
    for entry in dirs.into_iter().take(to_remove) {
        let _ = std::fs::remove_dir_all(entry.path());
    }
}

// ─── Changelog ──────────────────────────────────────────────────────────────

/// A changelog entry for one evolution transaction.
#[derive(serde::Serialize)]
struct ChangelogEntry {
    txn_id: String,
    ts: String,
    files: Vec<String>,
    changes: Vec<ChangeDetail>,
    reason: String,
}

#[derive(serde::Serialize)]
struct ChangeDetail {
    #[serde(rename = "type")]
    change_type: String,
    id: String,
}

/// Append an entry to `_versions/changelog.jsonl`.
pub fn append_changelog(
    chat_root: &Path,
    txn_id: &str,
    files: &[String],
    changes: &[(String, String)], // (type, id)
    reason: &str,
) -> Result<()> {
    let vdir = versions_dir(chat_root);
    std::fs::create_dir_all(&vdir)?;
    let path = vdir.join("changelog.jsonl");

    let entry = ChangelogEntry {
        txn_id: txn_id.to_string(),
        ts: chrono::Utc::now().to_rfc3339(),
        files: files.to_vec(),
        changes: changes
            .iter()
            .map(|(t, id)| ChangeDetail {
                change_type: t.clone(),
                id: id.clone(),
            })
            .collect(),
        reason: reason.to_string(),
    };

    let mut line = serde_json::to_string(&entry)?;
    line.push('\n');

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    file.write_all(line.as_bytes())?;
    Ok(())
}

// ─── Audit log (evolution.log JSONL) ────────────────────────────────────────

/// Append to the JSONL audit file, SQLite evolution_log table, and core audit log.
pub fn log_evolution_event(
    conn: &Connection,
    chat_root: &Path,
    event_type: &str,
    target_id: &str,
    reason: &str,
    txn_id: &str,
) -> Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();

    // SQLite
    conn.execute(
        "INSERT INTO evolution_log (ts, type, target_id, reason, version) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![ts, event_type, target_id, reason, txn_id],
    )?;

    // JSONL audit file (append-only)
    let log_path = chat_root.join("evolution.log");
    let entry = serde_json::json!({
        "ts": ts,
        "type": event_type,
        "id": target_id,
        "reason": reason,
        "txn_id": txn_id,
    });
    let mut line = serde_json::to_string(&entry)?;
    line.push('\n');
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    file.write_all(line.as_bytes())?;

    // EVO-5: Also write to the core audit log (SKILLLITE_AUDIT_LOG)
    skilllite_core::observability::audit_evolution_event(event_type, target_id, reason, txn_id);

    Ok(())
}

// ─── Mark decisions as evolved ──────────────────────────────────────────────

pub fn mark_decisions_evolved(conn: &Connection, ids: &[i64]) -> Result<()> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
    let sql = format!(
        "UPDATE decisions SET evolved = 1 WHERE id IN ({})",
        placeholders.join(",")
    );
    let mut stmt = conn.prepare(&sql)?;
    let params: Vec<Box<dyn rusqlite::types::ToSql>> =
        ids.iter().map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>).collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    stmt.execute(param_refs.as_slice())?;
    Ok(())
}

// ─── Run evolution (main entry point) ────────────────────────────────────────

/// Run a full evolution cycle. Called from idle trigger or shutdown hook.
///
/// Returns the txn_id if evolution actually produced changes, None otherwise.
pub async fn run_evolution(
    chat_root: &Path,
    api_base: &str,
    api_key: &str,
    model: &str,
) -> Result<Option<String>> {
    if !try_start_evolution() {
        tracing::debug!("Evolution already in progress, skipping");
        return Ok(None);
    }

    let result = run_evolution_inner(chat_root, api_base, api_key, model).await;

    finish_evolution();
    result
}

async fn run_evolution_inner(
    chat_root: &Path,
    api_base: &str,
    api_key: &str,
    model: &str,
) -> Result<Option<String>> {
    // Phase 1: synchronous DB reads (Connection is not Send, must not cross await)
    let (scope, txn_id, snapshot_files) = {
        let conn = feedback::open_evolution_db(chat_root)?;
        let scope = should_evolve(&conn)?;
        if !scope.prompts && !scope.memory && !scope.skills {
            mark_decisions_evolved(&conn, &scope.decision_ids)?;
            return Ok(None);
        }
        let txn_id = format!("evo_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
        tracing::info!(
            "Starting evolution txn={} (prompts={}, memory={}, skills={})",
            txn_id, scope.prompts, scope.memory, scope.skills
        );
        let snapshot_files = if scope.prompts {
            create_snapshot(chat_root, &txn_id, &[
                "rules.json", "examples.json",
                "planning.md", "execution.md", "system.md",
            ])?
        } else {
            Vec::new()
        };
        (scope, txn_id, snapshot_files)
        // conn dropped here
    };

    let mut all_changes: Vec<(String, String)> = Vec::new();
    let mut reason_parts: Vec<String> = Vec::new();

    // Phase 2: async LLM calls (no Connection held)
    if scope.prompts {
        match prompt_learner::evolve_prompts(
            chat_root, api_base, api_key, model, &txn_id,
        )
        .await
        {
            Ok(changes) => {
                if !changes.is_empty() {
                    reason_parts.push(format!("{} prompt changes", changes.len()));
                }
                all_changes.extend(changes);
            }
            Err(e) => {
                tracing::warn!("Prompt evolution failed: {}", e);
            }
        }
    }

    if scope.skills {
        let generate = matches!(scope.skill_action, SkillAction::Generate);
        match skill_synth::evolve_skills(
            chat_root, api_base, api_key, model, &txn_id, generate,
        )
        .await
        {
            Ok(changes) => {
                if !changes.is_empty() {
                    reason_parts.push(format!("{} skill changes", changes.len()));
                }
                all_changes.extend(changes);
            }
            Err(e) => {
                tracing::warn!("Skill evolution failed: {}", e);
            }
        }
    }

    // Phase 3: synchronous DB writes (re-open connection)
    {
        let conn = feedback::open_evolution_db(chat_root)?;

        // Log evolution events
        for (ctype, cid) in &all_changes {
            log_evolution_event(&conn, chat_root, ctype, cid, "prompt evolution", &txn_id)?;
        }

        // Update reusable status
        if scope.prompts {
            if let Err(e) = prompt_learner::update_reusable_status(&conn, chat_root) {
                tracing::warn!("Failed to update reusable status: {}", e);
            }
        }

        mark_decisions_evolved(&conn, &scope.decision_ids)?;
        let _ = feedback::update_daily_metrics(&conn);

        if all_changes.is_empty() {
            return Ok(None);
        }

        let reason = reason_parts.join("; ");
        append_changelog(chat_root, &txn_id, &snapshot_files, &all_changes, &reason)?;

        let decisions_path = chat_root.join("DECISIONS.md");
        let _ = feedback::export_decisions_md(&conn, &decisions_path);

        tracing::info!("Evolution txn={} complete: {}", txn_id, reason);
    }

    // EVO-6: External learning (optional, gated by SKILLLITE_EXTERNAL_LEARNING=1)
    match external_learner::run_external_learning(chat_root, api_base, api_key, model, &txn_id)
        .await
    {
        Ok(ext_changes) => {
            if !ext_changes.is_empty() {
                tracing::info!("EVO-6: {} external changes applied", ext_changes.len());
                all_changes.extend(ext_changes);
            }
        }
        Err(e) => {
            tracing::warn!("EVO-6 external learning failed (non-fatal): {}", e);
        }
    }

    Ok(Some(txn_id))
}

// ─── EVO-5: User-visible evolution event formatting ──────────────────────────

/// Format evolution changes into user-visible messages.
/// Returns a list of human-readable descriptions for display after evolution.
pub fn format_evolution_changes(changes: &[(String, String)]) -> Vec<String> {
    changes
        .iter()
        .filter_map(|(change_type, id)| {
            let msg = match change_type.as_str() {
                "rule_added" => format!("\u{1f4a1} 已学习新规则: {}", id),
                "rule_updated" => format!("\u{1f504} 已优化规则: {}", id),
                "rule_retired" => format!("\u{1f5d1}\u{fe0f} 已退役低效规则: {}", id),
                "example_added" => format!("\u{1f4d6} 已新增示例: {}", id),
                "skill_generated" => format!("\u{2728} 已自动生成 Skill: {}", id),
                "skill_pending" => format!("\u{1f4a1} 新 Skill {} 待确认（运行 `skilllite evolution confirm {}` 加入）", id, id),
                "skill_refined" => format!("\u{1f527} 已优化 Skill: {}", id),
                "skill_retired" => format!("\u{1f4e6} 已归档 Skill: {}", id),
                "auto_rollback" => format!("\u{26a0}\u{fe0f} 检测到质量下降，已自动回滚: {}", id),
                "reusable_promoted" => format!("\u{2b06}\u{fe0f} 规则晋升为通用: {}", id),
                "reusable_demoted" => format!("\u{2b07}\u{fe0f} 规则降级为低效: {}", id),
                // EVO-6: External learning change types
                "external_rule_added" => format!("\u{1f310} 已从外部来源学习规则: {}", id),
                "external_rule_promoted" => format!("\u{2b06}\u{fe0f} 外部规则晋升为优质: {}", id),
                "source_paused" => format!("\u{23f8}\u{fe0f} 信源可达性过低，已暂停: {}", id),
                "source_retired" => format!("\u{1f5d1}\u{fe0f} 已退役低质量信源: {}", id),
                "source_discovered" => format!("\u{1f50d} 发现新信源: {}", id),
                _ => return None,
            };
            Some(msg)
        })
        .collect()
}

// ─── Shutdown hook (lightweight, no LLM calls) ─────────────────────────────

/// Called on graceful shutdown. Marks pending data; does NOT call LLM.
pub fn on_shutdown(chat_root: &Path) {
    if !try_start_evolution() {
        return; // Evolution already running, skip
    }
    if let Ok(conn) = feedback::open_evolution_db(chat_root) {
        // Just flush daily metrics — LLM evolution deferred to next session
        let _ = feedback::update_daily_metrics(&conn);
        let _ = feedback::export_decisions_md(&conn, &chat_root.join("DECISIONS.md"));
    }
    finish_evolution();
}

// ─── Auto-rollback check ────────────────────────────────────────────────────

/// Check system metrics for degradation and auto-rollback if needed.
/// Triggered after each evolution cycle.
pub fn check_auto_rollback(conn: &Connection, chat_root: &Path) -> Result<bool> {
    // Check if first_success_rate declined for 3 consecutive days > 10%
    let mut stmt = conn.prepare(
        "SELECT date, first_success_rate, user_correction_rate
         FROM evolution_metrics
         WHERE date > date('now', '-5 days')
         ORDER BY date DESC LIMIT 4",
    )?;
    let metrics: Vec<(String, f64, f64)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .filter_map(|r| r.ok())
        .collect();

    if metrics.len() < 3 {
        return Ok(false); // Not enough data
    }

    // Check declining first_success_rate
    let fsr_declining = metrics.windows(2).take(3).all(|w| {
        w[0].1 < w[1].1 - 0.10 // each day > 10% decline
    });

    // Check rising user_correction_rate
    let ucr_rising = metrics.windows(2).take(3).all(|w| {
        w[0].2 > w[1].2 + 0.20 // each day > 20% rise
    });

    if fsr_declining || ucr_rising {
        let reason = if fsr_declining {
            "first_success_rate declined >10% for 3 consecutive days"
        } else {
            "user_correction_rate rose >20% for 3 consecutive days"
        };

        // Find the most recent evolution txn_id
        let last_txn: Option<String> = conn
            .query_row(
                "SELECT DISTINCT version FROM evolution_log
                 WHERE type NOT LIKE '%_rolled_back'
                 ORDER BY ts DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        if let Some(txn_id) = last_txn {
            tracing::warn!("Auto-rollback triggered: {} (txn={})", reason, txn_id);
            restore_snapshot(chat_root, &txn_id)?;

            // Mark rolled-back entries
            conn.execute(
                "UPDATE evolution_log SET type = type || '_rolled_back' WHERE version = ?1",
                params![txn_id],
            )?;

            log_evolution_event(
                conn,
                chat_root,
                "auto_rollback",
                &txn_id,
                reason,
                &format!("rollback_{}", txn_id),
            )?;

            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
        feedback::ensure_evolution_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn test_evolution_mutex() {
        assert!(try_start_evolution());
        assert!(!try_start_evolution()); // second attempt fails
        finish_evolution();
        assert!(try_start_evolution()); // after release, succeeds again
        finish_evolution();
    }

    #[test]
    fn test_atomic_write() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("test.json");
        atomic_write(&path, r#"{"hello": "world"}"#).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, r#"{"hello": "world"}"#);
    }

    #[test]
    fn test_gatekeeper_l1() {
        let root = Path::new("/home/user/.skilllite/chat");
        // Allowed: files in prompts/ (including templates — integrity checked separately)
        assert!(gatekeeper_l1_path(root, Path::new("/home/user/.skilllite/chat/prompts/rules.json")));
        assert!(gatekeeper_l1_path(root, Path::new("/home/user/.skilllite/chat/prompts/examples.json")));
        assert!(gatekeeper_l1_path(root, Path::new("/home/user/.skilllite/chat/prompts/planning.md")));
        assert!(gatekeeper_l1_path(root, Path::new("/home/user/.skilllite/chat/memory/foo.md")));
        assert!(gatekeeper_l1_path(root, Path::new("/home/user/.skilllite/chat/skills/_evolved/test")));
        // Blocked: outside allowed dirs
        assert!(!gatekeeper_l1_path(root, Path::new("/home/user/.skilllite/chat/sessions.json")));
        assert!(!gatekeeper_l1_path(root, Path::new("/etc/passwd")));
    }

    #[test]
    fn test_gatekeeper_l1b_template_integrity() {
        // Valid: has all required placeholders for planning.md
        let valid = "Plan for {{TODAY}} with {{RULES_SECTION}} and {{EXAMPLES_SECTION}} output to {{OUTPUT_DIR}}";
        assert!(gatekeeper_l1_template_integrity("planning.md", valid).is_ok());

        // Invalid: missing {{RULES_SECTION}} for planning.md
        let missing_rules = "Plan for {{TODAY}} output to {{OUTPUT_DIR}} and {{EXAMPLES_SECTION}}";
        assert!(gatekeeper_l1_template_integrity("planning.md", missing_rules).is_err());

        // Invalid: missing all placeholders
        assert!(gatekeeper_l1_template_integrity("planning.md", "just plain text").is_err());

        // Data files (no required placeholders) always pass
        assert!(gatekeeper_l1_template_integrity("rules.json", "anything").is_ok());

        // system.md has no required placeholders, always passes
        assert!(gatekeeper_l1_template_integrity("system.md", "any content").is_ok());
    }

    #[test]
    fn test_gatekeeper_l2() {
        assert!(gatekeeper_l2_size(3, 2, 1));
        assert!(gatekeeper_l2_size(5, 3, 1));
        assert!(!gatekeeper_l2_size(6, 0, 0));
        assert!(!gatekeeper_l2_size(0, 4, 0));
        assert!(!gatekeeper_l2_size(0, 0, 2));
    }

    #[test]
    fn test_gatekeeper_l3() {
        assert!(gatekeeper_l3_content("normal rule about coding").is_ok());
        assert!(gatekeeper_l3_content("use grep before editing").is_ok());
        assert!(gatekeeper_l3_content("contains api_key=abc123").is_err());
        assert!(gatekeeper_l3_content("skip scan for this").is_err());
        assert!(gatekeeper_l3_content("bypass security").is_err());
        assert!(gatekeeper_l3_content("set password=foo").is_err());
    }

    #[test]
    fn test_should_evolve_empty() {
        let conn = test_db();
        let scope = should_evolve_with_mode(&conn, EvolutionMode::All).unwrap();
        assert!(!scope.prompts);
        assert!(!scope.memory);
        assert!(!scope.skills);
    }

    #[test]
    fn test_should_evolve_prompts() {
        let conn = test_db();
        for i in 0..5 {
            conn.execute(
                "INSERT INTO decisions (session_id, total_tools, failed_tools, replans, task_completed)
                 VALUES (?1, 3, ?2, ?3, 1)",
                params![format!("s{}", i), if i < 2 { 1 } else { 0 }, if i < 2 { 1 } else { 0 }],
            ).unwrap();
        }
        let scope = should_evolve_with_mode(&conn, EvolutionMode::All).unwrap();
        assert!(scope.prompts, "should trigger prompt evolution with 5 meaningful + 2 failures");
        assert!(scope.memory, "should trigger memory evolution");
    }

    #[test]
    fn test_snapshot_and_restore() {
        let tmp = TempDir::new().unwrap();
        let chat_root = tmp.path();
        let prompts = chat_root.join("prompts");
        std::fs::create_dir_all(&prompts).unwrap();

        std::fs::write(prompts.join("rules.json"), r#"[{"id":"original"}]"#).unwrap();

        let backed = create_snapshot(chat_root, "evo_test_001", &["rules.json"]).unwrap();
        assert_eq!(backed, vec!["rules.json"]);

        // Modify file
        std::fs::write(prompts.join("rules.json"), r#"[{"id":"modified"}]"#).unwrap();

        // Restore
        restore_snapshot(chat_root, "evo_test_001").unwrap();
        let content = std::fs::read_to_string(prompts.join("rules.json")).unwrap();
        assert!(content.contains("original"));
    }

    #[test]
    fn test_changelog_append() {
        let tmp = TempDir::new().unwrap();
        let chat_root = tmp.path();

        append_changelog(
            chat_root,
            "evo_test_001",
            &["rules.json".into()],
            &[("rule_added".into(), "evo_test_rule".into())],
            "test reason",
        )
        .unwrap();

        let vdir = versions_dir(chat_root);
        let content = std::fs::read_to_string(vdir.join("changelog.jsonl")).unwrap();
        assert!(content.contains("evo_test_001"));
        assert!(content.contains("rule_added"));
    }

    #[test]
    fn test_mark_decisions_evolved() {
        let conn = test_db();
        conn.execute(
            "INSERT INTO decisions (session_id, total_tools, task_completed) VALUES ('s', 3, 1)",
            [],
        ).unwrap();
        let id: i64 = conn.last_insert_rowid();

        let evolved: bool = conn.query_row(
            "SELECT evolved FROM decisions WHERE id = ?1", params![id], |r| r.get(0)
        ).unwrap();
        assert!(!evolved);

        mark_decisions_evolved(&conn, &[id]).unwrap();

        let evolved: bool = conn.query_row(
            "SELECT evolved FROM decisions WHERE id = ?1", params![id], |r| r.get(0)
        ).unwrap();
        assert!(evolved);
    }

    #[test]
    fn test_log_evolution_event() {
        let tmp = TempDir::new().unwrap();
        let conn = test_db();

        log_evolution_event(
            &conn, tmp.path(),
            "rule_added", "evo_test_rule",
            "test reason", "evo_test_001",
        ).unwrap();

        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM evolution_log WHERE type = 'rule_added'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);

        // Check JSONL file
        let log = std::fs::read_to_string(tmp.path().join("evolution.log")).unwrap();
        assert!(log.contains("evo_test_rule"));
    }

    // ─── EVO-5 tests ────────────────────────────────────────────────────────

    #[test]
    fn test_evolution_mode_parsing() {
        // Default (no env var set) → All
        std::env::remove_var("SKILLLITE_EVOLUTION");
        let mode = EvolutionMode::from_env();
        assert_eq!(mode, EvolutionMode::All);
        assert!(!mode.is_disabled());
        assert!(mode.prompts_enabled());
        assert!(mode.memory_enabled());
        assert!(mode.skills_enabled());
    }

    #[test]
    fn test_evolution_mode_disabled() {
        std::env::set_var("SKILLLITE_EVOLUTION", "0");
        let mode = EvolutionMode::from_env();
        assert_eq!(mode, EvolutionMode::Disabled);
        assert!(mode.is_disabled());
        assert!(!mode.prompts_enabled());
        assert!(!mode.memory_enabled());
        assert!(!mode.skills_enabled());
        std::env::remove_var("SKILLLITE_EVOLUTION");
    }

    #[test]
    fn test_evolution_mode_prompts_only() {
        std::env::set_var("SKILLLITE_EVOLUTION", "prompts");
        let mode = EvolutionMode::from_env();
        assert_eq!(mode, EvolutionMode::PromptsOnly);
        assert!(mode.prompts_enabled());
        assert!(!mode.memory_enabled());
        assert!(!mode.skills_enabled());
        std::env::remove_var("SKILLLITE_EVOLUTION");
    }

    #[test]
    fn test_should_evolve_disabled() {
        let conn = test_db();
        for i in 0..5 {
            conn.execute(
                "INSERT INTO decisions (session_id, total_tools, failed_tools, replans, task_completed)
                 VALUES (?1, 3, ?2, ?3, 1)",
                params![format!("s{}", i), if i < 2 { 1 } else { 0 }, if i < 2 { 1 } else { 0 }],
            ).unwrap();
        }
        let scope = should_evolve_with_mode(&conn, EvolutionMode::Disabled).unwrap();
        assert!(!scope.prompts, "should NOT trigger when evolution disabled");
        assert!(!scope.memory, "should NOT trigger when evolution disabled");
        assert!(!scope.skills, "should NOT trigger when evolution disabled");
    }

    #[test]
    fn test_should_evolve_prompts_only_mode() {
        let conn = test_db();
        for i in 0..5 {
            conn.execute(
                "INSERT INTO decisions (session_id, total_tools, failed_tools, replans, task_completed)
                 VALUES (?1, 3, ?2, ?3, 1)",
                params![format!("s{}", i), if i < 2 { 1 } else { 0 }, if i < 2 { 1 } else { 0 }],
            ).unwrap();
        }
        let scope = should_evolve_with_mode(&conn, EvolutionMode::PromptsOnly).unwrap();
        assert!(scope.prompts, "prompts should trigger in prompts-only mode");
        assert!(!scope.memory, "memory should NOT trigger in prompts-only mode");
        assert!(!scope.skills, "skills should NOT trigger in prompts-only mode");
    }

    #[test]
    fn test_format_evolution_changes() {
        let changes = vec![
            ("rule_added".to_string(), "grep_first".to_string()),
            ("skill_pending".to_string(), "daily_report".to_string()),
            ("auto_rollback".to_string(), "evo_001".to_string()),
            ("unknown_type".to_string(), "x".to_string()),
        ];
        let messages = format_evolution_changes(&changes);
        assert_eq!(messages.len(), 3, "unknown_type should be filtered");
        assert!(messages[0].contains("grep_first"));
        assert!(messages[1].contains("daily_report"));
        assert!(messages[1].contains("待确认"));
        assert!(messages[1].contains("confirm"));
        assert!(messages[2].contains("evo_001"));
    }
}
