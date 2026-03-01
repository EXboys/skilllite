//! SkillLite Evolution: self-evolving prompts, skills, and memory.
//!
//! EVO-1: Feedback collection + evaluation system + structured memory.
//! EVO-2: Prompt externalization + seed data mechanism.
//! EVO-3: Evolution engine core + evolution prompt design.
//! EVO-5: Polish + transparency (audit, degradation, CLI, time trends).
//!
//! Interacts with the agent through the [`EvolutionLlm`] trait for LLM completion.

pub mod external_learner;
pub mod feedback;
pub mod prompt_learner;
pub mod seed;
pub mod skill_synth;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use rusqlite::{params, Connection};

// ─── EvolutionLlm trait: agent integration ────────────────────────────────────

/// Minimal message format for evolution LLM calls (no tool calling).
#[derive(Debug, Clone)]
pub struct EvolutionMessage {
    pub role: String,
    pub content: Option<String>,
}

impl EvolutionMessage {
    pub fn user(content: &str) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(content.to_string()),
        }
    }

    pub fn system(content: &str) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(content.to_string()),
        }
    }
}

/// LLM completion interface for evolution.
///
/// The agent implements this trait to provide LLM access. Evolution uses it
/// for prompt learning, skill synthesis, and external knowledge extraction.
#[async_trait::async_trait]
pub trait EvolutionLlm: Send + Sync {
    /// Non-streaming chat completion. Returns the assistant's text content.
    async fn complete(
        &self,
        messages: &[EvolutionMessage],
        model: &str,
        temperature: f64,
    ) -> Result<String>;
}

// ─── EVO-5: Evolution mode ───────────────────────────────────────────────────

/// Which dimensions of evolution are enabled.
#[derive(Debug, Clone, PartialEq)]
pub enum EvolutionMode {
    All,
    PromptsOnly,
    MemoryOnly,
    SkillsOnly,
    Disabled,
}

impl EvolutionMode {
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

// ─── SkillAction (used by should_evolve) ──────────────────────────────────────

/// Action type for skill evolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkillAction {
    #[default]
    None,
    Generate,
    Refine,
}

// ─── Concurrency: evolution mutex ────────────────────────────────────────────

static EVOLUTION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub fn try_start_evolution() -> bool {
    EVOLUTION_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

pub fn finish_evolution() {
    EVOLUTION_IN_PROGRESS.store(false, Ordering::SeqCst);
}

// ─── Atomic file writes ───────────────────────────────────────────────────────

pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ─── Evolution scope ──────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct EvolutionScope {
    pub skills: bool,
    pub skill_action: SkillAction,
    pub memory: bool,
    pub prompts: bool,
    pub decision_ids: Vec<i64>,
}

pub fn should_evolve(conn: &Connection) -> Result<EvolutionScope> {
    should_evolve_with_mode(conn, EvolutionMode::from_env())
}

pub fn should_evolve_with_mode(conn: &Connection, mode: EvolutionMode) -> Result<EvolutionScope> {
    if mode.is_disabled() {
        return Ok(EvolutionScope::default());
    }

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
        return Ok(EvolutionScope::default());
    }

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

    let mut stmt = conn.prepare("SELECT id FROM decisions WHERE evolved = 0")?;
    let ids: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

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

    if mode.skills_enabled() && meaningful >= 3 && (failures > 0 || repeated_patterns > 0) {
        scope.skills = true;
        scope.skill_action = if repeated_patterns > 0 {
            SkillAction::Generate
        } else {
            SkillAction::Refine
        };
    }

    if mode.memory_enabled() && meaningful >= 3 {
        scope.memory = true;
    }

    if mode.prompts_enabled() && meaningful >= 5 && (failures >= 2 || replans >= 2) {
        scope.prompts = true;
    }

    Ok(scope)
}

// ─── Gatekeeper (L1-L3) ───────────────────────────────────────────────────────

const ALLOWED_EVOLUTION_PATHS: &[&str] = &["prompts", "memory", "skills/_evolved"];

pub fn gatekeeper_l1_path(chat_root: &Path, target: &Path) -> bool {
    for allowed in ALLOWED_EVOLUTION_PATHS {
        let allowed_dir = chat_root.join(allowed);
        if target.starts_with(&allowed_dir) {
            return true;
        }
    }
    false
}

pub fn gatekeeper_l1_template_integrity(filename: &str, new_content: &str) -> Result<()> {
    let missing = seed::validate_template(filename, new_content);
    if !missing.is_empty() {
        anyhow::bail!(
            "Gatekeeper L1b: evolved template '{}' is missing required placeholders {:?}",
            filename,
            missing
        );
    }
    Ok(())
}

pub fn gatekeeper_l2_size(new_rules: usize, new_examples: usize, new_skills: usize) -> bool {
    new_rules <= 5 && new_examples <= 3 && new_skills <= 1
}

const SENSITIVE_PATTERNS: &[&str] = &[
    "api_key", "api-key", "apikey",
    "secret", "password", "passwd",
    "token", "bearer",
    "private_key", "private-key",
    "-----BEGIN", "-----END",
    "skip scan", "bypass", "disable security",
    "eval(", "exec(", "__import__",
];

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

// ─── Snapshots ────────────────────────────────────────────────────────────────

fn versions_dir(chat_root: &Path) -> std::path::PathBuf {
    chat_root.join("prompts").join("_versions")
}

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
    prune_snapshots(chat_root, 10);
    Ok(backed_up)
}

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

// ─── Changelog ───────────────────────────────────────────────────────────────

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

pub fn append_changelog(
    chat_root: &Path,
    txn_id: &str,
    files: &[String],
    changes: &[(String, String)],
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

// ─── Audit log ───────────────────────────────────────────────────────────────

pub fn log_evolution_event(
    conn: &Connection,
    chat_root: &Path,
    event_type: &str,
    target_id: &str,
    reason: &str,
    txn_id: &str,
) -> Result<()> {
    let ts = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO evolution_log (ts, type, target_id, reason, version) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![ts, event_type, target_id, reason, txn_id],
    )?;

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

    skilllite_core::observability::audit_evolution_event(event_type, target_id, reason, txn_id);

    Ok(())
}

// ─── Mark decisions evolved ───────────────────────────────────────────────────

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

// ─── Run evolution (main entry point) ──────────────────────────────────────────

/// Run a full evolution cycle.
///
/// Returns the txn_id if evolution produced changes, None otherwise.
pub async fn run_evolution<L: EvolutionLlm>(
    chat_root: &Path,
    llm: &L,
    api_base: &str,
    api_key: &str,
    model: &str,
) -> Result<Option<String>> {
    if !try_start_evolution() {
        return Ok(None);
    }

    let result = run_evolution_inner(chat_root, llm, api_base, api_key, model).await;

    finish_evolution();
    result
}

async fn run_evolution_inner<L: EvolutionLlm>(
    chat_root: &Path,
    llm: &L,
    _api_base: &str,
    _api_key: &str,
    model: &str,
) -> Result<Option<String>> {
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
    };

    let mut all_changes: Vec<(String, String)> = Vec::new();
    let mut reason_parts: Vec<String> = Vec::new();

    if scope.prompts {
        match prompt_learner::evolve_prompts(chat_root, llm, model, &txn_id).await {
            Ok(changes) => {
                if !changes.is_empty() {
                    reason_parts.push(format!("{} prompt changes", changes.len()));
                }
                all_changes.extend(changes);
            }
            Err(e) => tracing::warn!("Prompt evolution failed: {}", e),
        }
    }

    if scope.skills {
        let generate = matches!(scope.skill_action, SkillAction::Generate);
        match skill_synth::evolve_skills(chat_root, llm, model, &txn_id, generate).await {
            Ok(changes) => {
                if !changes.is_empty() {
                    reason_parts.push(format!("{} skill changes", changes.len()));
                }
                all_changes.extend(changes);
            }
            Err(e) => tracing::warn!("Skill evolution failed: {}", e),
        }
    }

    {
        let conn = feedback::open_evolution_db(chat_root)?;

        for (ctype, cid) in &all_changes {
            log_evolution_event(&conn, chat_root, ctype, cid, "prompt evolution", &txn_id)?;
        }

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

    match external_learner::run_external_learning(chat_root, llm, model, &txn_id).await {
        Ok(ext_changes) => {
            if !ext_changes.is_empty() {
                tracing::info!("EVO-6: {} external changes applied", ext_changes.len());
                all_changes.extend(ext_changes);
            }
        }
        Err(e) => tracing::warn!("EVO-6 external learning failed (non-fatal): {}", e),
    }

    Ok(Some(txn_id))
}

pub fn query_changes_by_txn(conn: &Connection, txn_id: &str) -> Vec<(String, String)> {
    let mut stmt = match conn.prepare(
        "SELECT type, target_id FROM evolution_log WHERE version = ?1",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map(params![txn_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        ))
    })
    .ok()
    .into_iter()
    .flatten()
    .filter_map(|r| r.ok())
    .collect()
}

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

// ─── Shutdown hook ────────────────────────────────────────────────────────────

pub fn on_shutdown(chat_root: &Path) {
    if !try_start_evolution() {
        return;
    }
    if let Ok(conn) = feedback::open_evolution_db(chat_root) {
        let _ = feedback::update_daily_metrics(&conn);
        let _ = feedback::export_decisions_md(&conn, &chat_root.join("DECISIONS.md"));
    }
    finish_evolution();
}

// ─── Auto-rollback ───────────────────────────────────────────────────────────

pub fn check_auto_rollback(conn: &Connection, chat_root: &Path) -> Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT date, first_success_rate, user_correction_rate
         FROM evolution_metrics
         WHERE date > date('now', '-5 days')
         ORDER BY date DESC LIMIT 4",
    )?;
    let metrics: Vec<(String, f64, f64)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .filter_map(|r| r.ok())
        .collect();

    if metrics.len() < 3 {
        return Ok(false);
    }

    let fsr_declining = metrics.windows(2).take(3).all(|w| w[0].1 < w[1].1 - 0.10);
    let ucr_rising = metrics.windows(2).take(3).all(|w| w[0].2 > w[1].2 + 0.20);

    if fsr_declining || ucr_rising {
        let reason = if fsr_declining {
            "first_success_rate declined >10% for 3 consecutive days"
        } else {
            "user_correction_rate rose >20% for 3 consecutive days"
        };

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
