//! SkillLite Evolution: self-evolving prompts, skills, and memory.
//!
//! EVO-1: Feedback collection + evaluation system + structured memory.
//! EVO-2: Prompt externalization + seed data mechanism.
//! EVO-3: Evolution engine core + evolution prompt design.
//! EVO-5: Polish + transparency (audit, degradation, CLI, time trends).
//!
//! Interacts with the agent through the [`EvolutionLlm`] trait for LLM completion.

pub mod error;
pub mod external_learner;
pub mod feedback;
pub mod memory_learner;
pub mod prompt_learner;
pub mod seed;
pub mod skill_synth;

pub use error::{Error, Result};

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::bail;
use rusqlite::{params, Connection};
use skilllite_core::config::env_keys::evolution as evo_keys;

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

// ─── LLM response post-processing ────────────────────────────────────────────

/// Strip reasoning/thinking blocks emitted by various models.
/// Handles `<think>`, `<thinking>`, `<reasoning>` tags (DeepSeek, QwQ, open-source variants).
/// Returns the content after the last closing tag, or the original string if none found.
/// Should be called at the LLM layer so all downstream consumers get clean output.
pub fn strip_think_blocks(content: &str) -> &str {
    const CLOSING_TAGS: &[&str] = &["</think>", "</thinking>", "</reasoning>"];
    const OPENING_TAGS: &[&str] = &[
        "<think>",
        "<think\n",
        "<thinking>",
        "<thinking\n",
        "<reasoning>",
        "<reasoning\n",
    ];

    // Case 1: find the last closing tag, take content after it
    let mut best_end: Option<usize> = None;
    for tag in CLOSING_TAGS {
        if let Some(pos) = content.rfind(tag) {
            let end = pos + tag.len();
            if best_end.is_none_or(|bp| end > bp) {
                best_end = Some(end);
            }
        }
    }
    if let Some(end) = best_end {
        let after = content[end..].trim();
        if !after.is_empty() {
            return after;
        }
    }

    // Case 2: unclosed think tag (model hit token limit mid-thought).
    // Take content before the opening tag if it contains useful text.
    if best_end.is_none() {
        for tag in OPENING_TAGS {
            if let Some(pos) = content.find(tag) {
                let before = content[..pos].trim();
                if !before.is_empty() {
                    return before;
                }
            }
        }
    }

    content
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

/// Result of attempting to run evolution. Distinguishes "skipped (busy)" from "no scope" from "ran (with or without changes)".
#[derive(Debug, Clone)]
pub enum EvolutionRunResult {
    /// Another evolution run was already in progress; this invocation did not run.
    SkippedBusy,
    /// No evolution scope (e.g. thresholds not met, or evolution disabled).
    NoScope,
    /// Evolution ran. `Some(txn_id)` if changes were produced, `None` if run completed with no changes.
    Completed(Option<String>),
}

impl EvolutionRunResult {
    /// Returns the txn_id if evolution completed with changes.
    pub fn txn_id(&self) -> Option<&str> {
        match self {
            Self::Completed(Some(id)) => Some(id.as_str()),
            _ => None,
        }
    }
}

// ─── Atomic file writes (re-export from skilllite-fs) ─────────────────────────

pub use skilllite_fs::atomic_write;

// ─── 5.2 进化触发条件（从环境变量读取，默认与原硬编码一致）────────────────────────

/// 进化触发阈值，均由环境变量配置，未设置时使用下列默认值。
#[derive(Debug, Clone)]
pub struct EvolutionThresholds {
    pub cooldown_hours: f64,
    pub recent_days: i64,
    pub recent_limit: i64,
    pub meaningful_min_tools: i64,
    pub meaningful_threshold_skills: i64,
    pub meaningful_threshold_memory: i64,
    pub meaningful_threshold_prompts: i64,
    pub failures_min_prompts: i64,
    pub replans_min_prompts: i64,
    pub repeated_pattern_min_count: i64,
    pub repeated_pattern_min_success_rate: f64,
}

impl Default for EvolutionThresholds {
    fn default() -> Self {
        Self {
            cooldown_hours: 1.0,
            recent_days: 7,
            recent_limit: 100,
            meaningful_min_tools: 2,
            meaningful_threshold_skills: 3,
            meaningful_threshold_memory: 3,
            meaningful_threshold_prompts: 5,
            failures_min_prompts: 2,
            replans_min_prompts: 2,
            repeated_pattern_min_count: 3,
            repeated_pattern_min_success_rate: 0.8,
        }
    }
}

/// 进化触发场景：不设或 default 时与原有默认行为完全一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvolutionProfile {
    /// 与不设 EVO_PROFILE 时一致（当前默认阈值）
    Default,
    /// 演示/内测：冷却短、阈值低，进化更频繁
    Demo,
    /// 生产/省成本：冷却长、阈值高，进化更少
    Conservative,
}

impl EvolutionThresholds {
    /// 预设：演示场景，进化更频繁
    fn demo_preset() -> Self {
        Self {
            cooldown_hours: 0.25,
            recent_days: 3,
            recent_limit: 50,
            meaningful_min_tools: 1,
            meaningful_threshold_skills: 1,
            meaningful_threshold_memory: 1,
            meaningful_threshold_prompts: 2,
            failures_min_prompts: 1,
            replans_min_prompts: 1,
            repeated_pattern_min_count: 2,
            repeated_pattern_min_success_rate: 0.7,
        }
    }

    /// 预设：保守场景，进化更少、省成本
    fn conservative_preset() -> Self {
        Self {
            cooldown_hours: 4.0,
            recent_days: 14,
            recent_limit: 200,
            meaningful_min_tools: 2,
            meaningful_threshold_skills: 5,
            meaningful_threshold_memory: 5,
            meaningful_threshold_prompts: 8,
            failures_min_prompts: 3,
            replans_min_prompts: 3,
            repeated_pattern_min_count: 4,
            repeated_pattern_min_success_rate: 0.85,
        }
    }

    pub fn from_env() -> Self {
        let parse_i64 = |key: &str, default: i64| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let parse_f64 = |key: &str, default: f64| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        let profile = match std::env::var(evo_keys::SKILLLITE_EVO_PROFILE)
            .ok()
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some("demo") => EvolutionProfile::Demo,
            Some("conservative") => EvolutionProfile::Conservative,
            _ => EvolutionProfile::Default,
        };
        let base = match profile {
            EvolutionProfile::Default => Self::default(),
            EvolutionProfile::Demo => Self::demo_preset(),
            EvolutionProfile::Conservative => Self::conservative_preset(),
        };
        Self {
            cooldown_hours: parse_f64(evo_keys::SKILLLITE_EVO_COOLDOWN_HOURS, base.cooldown_hours),
            recent_days: parse_i64(evo_keys::SKILLLITE_EVO_RECENT_DAYS, base.recent_days),
            recent_limit: parse_i64(evo_keys::SKILLLITE_EVO_RECENT_LIMIT, base.recent_limit),
            meaningful_min_tools: parse_i64(
                evo_keys::SKILLLITE_EVO_MEANINGFUL_MIN_TOOLS,
                base.meaningful_min_tools,
            ),
            meaningful_threshold_skills: parse_i64(
                evo_keys::SKILLLITE_EVO_MEANINGFUL_THRESHOLD_SKILLS,
                base.meaningful_threshold_skills,
            ),
            meaningful_threshold_memory: parse_i64(
                evo_keys::SKILLLITE_EVO_MEANINGFUL_THRESHOLD_MEMORY,
                base.meaningful_threshold_memory,
            ),
            meaningful_threshold_prompts: parse_i64(
                evo_keys::SKILLLITE_EVO_MEANINGFUL_THRESHOLD_PROMPTS,
                base.meaningful_threshold_prompts,
            ),
            failures_min_prompts: parse_i64(
                evo_keys::SKILLLITE_EVO_FAILURES_MIN_PROMPTS,
                base.failures_min_prompts,
            ),
            replans_min_prompts: parse_i64(
                evo_keys::SKILLLITE_EVO_REPLANS_MIN_PROMPTS,
                base.replans_min_prompts,
            ),
            repeated_pattern_min_count: parse_i64(
                evo_keys::SKILLLITE_EVO_REPEATED_PATTERN_MIN_COUNT,
                base.repeated_pattern_min_count,
            ),
            repeated_pattern_min_success_rate: parse_f64(
                evo_keys::SKILLLITE_EVO_REPEATED_PATTERN_MIN_SUCCESS_RATE,
                base.repeated_pattern_min_success_rate,
            ),
        }
    }
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

impl EvolutionScope {
    /// 返回用于 evolution_run 日志展示的「进化方向」中文描述（供 evotown 等前端展示）
    pub fn direction_label(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.prompts {
            parts.push("规则与示例");
        }
        if self.skills {
            parts.push("技能");
        }
        if self.memory {
            parts.push("记忆");
        }
        if parts.is_empty() {
            return String::new();
        }
        parts.join("、")
    }
}

pub fn should_evolve(conn: &Connection) -> Result<EvolutionScope> {
    should_evolve_impl(conn, EvolutionMode::from_env(), false)
}

pub fn should_evolve_with_mode(conn: &Connection, mode: EvolutionMode) -> Result<EvolutionScope> {
    should_evolve_impl(conn, mode, false)
}

/// When force=true (e.g. manual `skilllite evolution run`), bypass decision thresholds.
fn should_evolve_impl(
    conn: &Connection,
    mode: EvolutionMode,
    force: bool,
) -> Result<EvolutionScope> {
    if mode.is_disabled() {
        return Ok(EvolutionScope::default());
    }

    let thresholds = EvolutionThresholds::from_env();

    let today_evolutions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evolution_log WHERE date(ts) = date('now')",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    let max_per_day: i64 = std::env::var(evo_keys::SKILLLITE_MAX_EVOLUTIONS_PER_DAY)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);
    if today_evolutions >= max_per_day {
        return Ok(EvolutionScope::default());
    }

    if !force {
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
        if last_evo_hours < thresholds.cooldown_hours {
            return Ok(EvolutionScope::default());
        }
    }

    let recent_condition = format!("ts >= datetime('now', '-{} days')", thresholds.recent_days);
    let recent_limit = thresholds.recent_limit;

    let (meaningful, failures, replans): (i64, i64, i64) = conn.query_row(
        &format!(
            "SELECT
                COUNT(CASE WHEN total_tools >= {} THEN 1 END),
                COUNT(CASE WHEN failed_tools > 0 THEN 1 END),
                COUNT(CASE WHEN replans > 0 THEN 1 END)
             FROM decisions WHERE {}",
            thresholds.meaningful_min_tools, recent_condition
        ),
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    let mut stmt = conn.prepare(&format!(
        "SELECT id FROM decisions WHERE {} ORDER BY ts DESC LIMIT {}",
        recent_condition, recent_limit
    ))?;
    let ids: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    // Group by tool_sequence_key (new) when available; fall back to task_description for
    // older decisions that predate the tool_sequence_key column.
    // COALESCE(NULLIF(key,''), desc) ensures empty-string keys also fall back.
    let repeated_patterns: i64 = conn
        .query_row(
            &format!(
                "SELECT COUNT(*) FROM (
                SELECT COALESCE(NULLIF(tool_sequence_key, ''), task_description) AS pattern_key,
                       COUNT(*) AS cnt,
                       SUM(CASE WHEN task_completed = 1 THEN 1 ELSE 0 END) AS successes
                FROM decisions
                WHERE {} AND (tool_sequence_key IS NOT NULL OR task_description IS NOT NULL)
                  AND total_tools >= 1
                GROUP BY pattern_key
                HAVING cnt >= {} AND CAST(successes AS REAL) / cnt >= {}
            )",
                recent_condition,
                thresholds.repeated_pattern_min_count,
                thresholds.repeated_pattern_min_success_rate
            ),
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let mut scope = EvolutionScope {
        decision_ids: ids.clone(),
        ..Default::default()
    };

    if force && !ids.is_empty() {
        // Manual trigger: bypass thresholds, enable all enabled modes
        if mode.skills_enabled() {
            scope.skills = true;
            scope.skill_action = if repeated_patterns > 0 {
                SkillAction::Generate
            } else {
                SkillAction::Refine
            };
        }
        if mode.memory_enabled() {
            scope.memory = true;
        }
        if mode.prompts_enabled() {
            scope.prompts = true;
        }
    } else {
        if mode.skills_enabled()
            && meaningful >= thresholds.meaningful_threshold_skills
            && (failures > 0 || repeated_patterns > 0)
        {
            scope.skills = true;
            scope.skill_action = if repeated_patterns > 0 {
                SkillAction::Generate
            } else {
                SkillAction::Refine
            };
        }
        if mode.memory_enabled() && meaningful >= thresholds.meaningful_threshold_memory {
            scope.memory = true;
        }
        if mode.prompts_enabled()
            && meaningful >= thresholds.meaningful_threshold_prompts
            && (failures >= thresholds.failures_min_prompts
                || replans >= thresholds.replans_min_prompts)
        {
            scope.prompts = true;
        }
    }

    Ok(scope)
}

// ─── Gatekeeper (L1-L3) ───────────────────────────────────────────────────────

const ALLOWED_EVOLUTION_PATHS: &[&str] = &["prompts", "memory", "skills/_evolved"];

/// L1 path gatekeeper. When skills_root is Some, also allows target under skills_root/_evolved
/// (project-level skill evolution).
pub fn gatekeeper_l1_path(chat_root: &Path, target: &Path, skills_root: Option<&Path>) -> bool {
    for allowed in ALLOWED_EVOLUTION_PATHS {
        let allowed_dir = chat_root.join(allowed);
        if target.starts_with(&allowed_dir) {
            return true;
        }
    }
    if let Some(sr) = skills_root {
        let evolved = sr.join("_evolved");
        if target.starts_with(&evolved) {
            return true;
        }
    }
    false
}

pub fn gatekeeper_l1_template_integrity(filename: &str, new_content: &str) -> Result<()> {
    let missing = seed::validate_template(filename, new_content);
    if !missing.is_empty() {
        bail!(
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
    "api_key",
    "api-key",
    "apikey",
    "secret",
    "password",
    "passwd",
    "token",
    "bearer",
    "private_key",
    "private-key",
    "-----BEGIN",
    "-----END",
    "skip scan",
    "bypass",
    "disable security",
    "eval(",
    "exec(",
    "__import__",
];

pub fn gatekeeper_l3_content(content: &str) -> Result<()> {
    let lower = content.to_lowercase();
    for pattern in SENSITIVE_PATTERNS {
        if lower.contains(pattern) {
            bail!(
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

/// How many evolution txn snapshot directories to keep under `prompts/_versions/`.
/// `0` = keep all (no pruning). Default `10`. Invalid env falls back to default.
fn evolution_snapshot_keep_count() -> usize {
    match std::env::var(evo_keys::SKILLLITE_EVOLUTION_SNAPSHOT_KEEP)
        .ok()
        .as_deref()
    {
        Some(s) if !s.is_empty() => s.parse::<usize>().unwrap_or(10),
        _ => 10,
    }
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
    prune_snapshots(chat_root, evolution_snapshot_keep_count());
    Ok(backed_up)
}

pub fn restore_snapshot(chat_root: &Path, txn_id: &str) -> Result<()> {
    let snap_dir = versions_dir(chat_root).join(txn_id);
    if !snap_dir.exists() {
        bail!("Snapshot not found: {}", txn_id);
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
    if keep == 0 {
        return;
    }
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
    let params: Vec<Box<dyn rusqlite::types::ToSql>> = ids
        .iter()
        .map(|id| Box::new(*id) as Box<dyn rusqlite::types::ToSql>)
        .collect();
    let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    stmt.execute(param_refs.as_slice())?;
    Ok(())
}

// ─── Run evolution (main entry point) ──────────────────────────────────────────

/// Run a full evolution cycle.
///
/// Returns [EvolutionRunResult]: SkippedBusy if another run in progress, NoScope if nothing to evolve, Completed(txn_id) otherwise.
/// When force=true (manual trigger), bypass decision thresholds.
/// skills_root: project-level dir (workspace/.skills). When None, skips skill evolution.
pub async fn run_evolution<L: EvolutionLlm>(
    chat_root: &Path,
    skills_root: Option<&Path>,
    llm: &L,
    api_base: &str,
    api_key: &str,
    model: &str,
    force: bool,
) -> Result<EvolutionRunResult> {
    if !try_start_evolution() {
        return Ok(EvolutionRunResult::SkippedBusy);
    }

    let result =
        run_evolution_inner(chat_root, skills_root, llm, api_base, api_key, model, force).await;

    finish_evolution();
    result
}

async fn run_evolution_inner<L: EvolutionLlm>(
    chat_root: &Path,
    skills_root: Option<&Path>,
    llm: &L,
    _api_base: &str,
    _api_key: &str,
    model: &str,
    force: bool,
) -> Result<EvolutionRunResult> {
    let conn = feedback::open_evolution_db(chat_root)?;
    let scope = should_evolve_impl(&conn, EvolutionMode::from_env(), force)?;
    if !scope.prompts && !scope.memory && !scope.skills {
        return Ok(EvolutionRunResult::NoScope);
    }
    let txn_id = format!("evo_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    tracing::info!(
        "Starting evolution txn={} (prompts={}, memory={}, skills={})",
        txn_id,
        scope.prompts,
        scope.memory,
        scope.skills
    );
    let snapshot_files = if scope.prompts {
        create_snapshot(
            chat_root,
            &txn_id,
            &[
                "rules.json",
                "examples.json",
                "planning.md",
                "execution.md",
                "system.md",
            ],
        )?
    } else {
        Vec::new()
    };

    // Drop conn before async work (Connection is !Send, cannot hold across .await).
    drop(conn);

    let mut all_changes: Vec<(String, String)> = Vec::new();
    let mut reason_parts: Vec<String> = Vec::new();

    // Run prompts / skills / memory evolution in parallel. Each module uses block_in_place
    // to batch its DB operations (one open per module), so we get both parallelism and fewer opens.
    let (prompt_res, skills_res, memory_res) = tokio::join!(
        async {
            if scope.prompts {
                prompt_learner::evolve_prompts(chat_root, llm, model, &txn_id).await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if scope.skills {
                let generate = true;
                skill_synth::evolve_skills(
                    chat_root,
                    skills_root,
                    llm,
                    model,
                    &txn_id,
                    generate,
                    force,
                )
                .await
            } else {
                Ok(Vec::new())
            }
        },
        async {
            if scope.memory {
                memory_learner::evolve_memory(chat_root, llm, model, &txn_id).await
            } else {
                Ok(Vec::new())
            }
        },
    );

    if scope.prompts {
        match prompt_res {
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
        match skills_res {
            Ok(changes) => {
                if !changes.is_empty() {
                    reason_parts.push(format!("{} skill changes", changes.len()));
                }
                all_changes.extend(changes);
            }
            Err(e) => tracing::warn!("Skill evolution failed: {}", e),
        }
    }
    if scope.memory {
        match memory_res {
            Ok(changes) => {
                if !changes.is_empty() {
                    reason_parts.push(format!("{} memory knowledge update(s)", changes.len()));
                }
                all_changes.extend(changes);
            }
            Err(e) => tracing::warn!("Memory evolution failed: {}", e),
        }
    }

    // Run external learning before changelog so its changes and modified files are in the same txn entry.
    match external_learner::run_external_learning(chat_root, llm, model, &txn_id).await {
        Ok(ext_changes) => {
            if !ext_changes.is_empty() {
                tracing::info!("EVO-6: {} external changes applied", ext_changes.len());
                reason_parts.push(format!("{} external change(s)", ext_changes.len()));
                all_changes.extend(ext_changes);
            }
        }
        Err(e) => tracing::warn!("EVO-6 external learning failed (non-fatal): {}", e),
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
        let auto_rolled_back = check_auto_rollback(&conn, chat_root)?;
        if auto_rolled_back {
            tracing::info!("EVO: auto-rollback triggered for txn={}", txn_id);
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_judgement",
                "rollback",
                "Auto-rollback triggered due to performance degradation",
                &txn_id,
            );
        } else {
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_judgement",
                "no_rollback",
                "No auto-rollback triggered",
                &txn_id,
            );
        }
        // let _ = feedback::export_judgement(&conn, &chat_root.join("JUDGEMENT.md")); // Removed for refactor
        if let Ok(Some(summary)) = feedback::build_latest_judgement(&conn) {
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_judgement",
                summary.judgement.as_str(),
                &summary.reason,
                &txn_id,
            );
            // Insert new judgement output to file here
            let judgement_output = format!(
                "## Evolution Judgement\n\n**Judgement:** {}\n\n**Reason:** {}\n",
                summary.judgement.as_str(),
                summary.reason
            );
            let judgement_path = chat_root.join("JUDGEMENT.md");
            if let Err(e) = skilllite_fs::atomic_write(&judgement_path, &judgement_output) {
                tracing::warn!("Failed to write JUDGEMENT.md: {}", e);
            }
        }

        if all_changes.is_empty() {
            // 即使无变更也记录一次，便于前端时间线展示进化运行记录（含本轮选择的进化方向）
            let dir = scope.direction_label();
            let reason = if dir.is_empty() {
                "进化运行完成，无新规则/技能产出".to_string()
            } else {
                format!("方向: {}；进化运行完成，无新规则/技能产出", dir)
            };
            let _ = log_evolution_event(&conn, chat_root, "evolution_run", "run", &reason, &txn_id);
            return Ok(EvolutionRunResult::Completed(None));
        }

        let dir = scope.direction_label();
        let reason = if dir.is_empty() {
            reason_parts.join("; ")
        } else {
            format!("方向: {}；{}", dir, reason_parts.join("; "))
        };
        // 记录本轮进化运行（含方向），便于前端时间线统一展示
        let _ = log_evolution_event(&conn, chat_root, "evolution_run", "run", &reason, &txn_id);

        // 只记录内容真正发生变化的文件：用快照与当前版本逐一对比。
        // snapshot_files 是进化前备份的全量清单，但实际修改的往往只是其中一部分
        // （如 rules.json / examples.json），planning.md 等通常未被触碰。
        let snap_dir = versions_dir(chat_root).join(&txn_id);
        let prompts_dir = chat_root.join("prompts");
        let mut modified_files: Vec<String> = snapshot_files
            .iter()
            .filter(|fname| {
                let snap_path = snap_dir.join(fname);
                let curr_path = prompts_dir.join(fname);
                match (std::fs::read(&snap_path), std::fs::read(&curr_path)) {
                    (Ok(old), Ok(new)) => old != new,
                    _ => false,
                }
            })
            .cloned()
            .collect();

        // External learner writes to prompts/rules.json; include it when external merged/promoted rules but snapshot didn't cover it (e.g. no scope.prompts).
        if all_changes
            .iter()
            .any(|(t, _)| t == "external_rule_added" || t == "external_rule_promoted")
        {
            const EXTERNAL_RULES_FILE: &str = "rules.json";
            if !modified_files.iter().any(|f| f == EXTERNAL_RULES_FILE) {
                let rules_path = prompts_dir.join(EXTERNAL_RULES_FILE);
                if rules_path.exists() {
                    modified_files.push(EXTERNAL_RULES_FILE.to_string());
                }
            }
        }

        append_changelog(chat_root, &txn_id, &modified_files, &all_changes, &reason)?;

        let _decisions_path = chat_root.join("DECISIONS.md");
        // let _ = feedback::export_decisions_md(&conn, &decisions_path); // Removed for refactor

        tracing::info!("Evolution txn={} complete: {}", txn_id, reason);
    }

    Ok(EvolutionRunResult::Completed(Some(txn_id)))
}

pub fn query_changes_by_txn(conn: &Connection, txn_id: &str) -> Vec<(String, String)> {
    let mut stmt =
        match conn.prepare("SELECT type, target_id FROM evolution_log WHERE version = ?1") {
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
                "skill_pending" => format!(
                    "\u{1f4a1} 新 Skill {} 待确认（运行 `skilllite evolution confirm {}` 加入）",
                    id, id
                ),
                "skill_refined" => format!("\u{1f527} 已优化 Skill: {}", id),
                "skill_retired" => format!("\u{1f4e6} 已归档 Skill: {}", id),
                "evolution_judgement" => {
                    let label = match id.as_str() {
                        "promote" => "保留",
                        "keep_observing" => "继续观察",
                        "rollback" => "回滚",
                        _ => id,
                    };
                    format!("\u{1f9ed} 本轮判断: {}", label)
                }
                "auto_rollback" => format!("\u{26a0}\u{fe0f} 检测到质量下降，已自动回滚: {}", id),
                "reusable_promoted" => format!("\u{2b06}\u{fe0f} 规则晋升为通用: {}", id),
                "reusable_demoted" => format!("\u{2b07}\u{fe0f} 规则降级为低效: {}", id),
                "external_rule_added" => format!("\u{1f310} 已从外部来源学习规则: {}", id),
                "external_rule_promoted" => format!("\u{2b06}\u{fe0f} 外部规则晋升为优质: {}", id),
                "source_paused" => format!("\u{23f8}\u{fe0f} 信源可达性过低，已暂停: {}", id),
                "source_retired" => format!("\u{1f5d1}\u{fe0f} 已退役低质量信源: {}", id),
                "source_discovered" => format!("\u{1f50d} 发现新信源: {}", id),
                "memory_knowledge_added" => format!("\u{1f4da} 已沉淀知识库（实体与关系）: {}", id),
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
        // let _ = feedback::export_decisions_md(&conn, &chat_root.join("DECISIONS.md")); // Removed for refactor
    }
    finish_evolution();
}

// ─── Auto-rollback ───────────────────────────────────────────────────────────

/// Executes the rollback actions (restoring snapshot, logging).
fn execute_evolution_rollback(
    conn: &Connection,
    chat_root: &Path,
    txn_id: &str,
    reason: &str,
) -> Result<()> {
    tracing::warn!("Evolution rollback executed: {} (txn={})", reason, txn_id);
    restore_snapshot(chat_root, txn_id)?;

    conn.execute(
        "UPDATE evolution_log SET type = type || '_rolled_back' WHERE version = ?1",
        params![txn_id],
    )?;

    log_evolution_event(
        conn,
        chat_root,
        "auto_rollback",
        txn_id,
        reason,
        &format!("rollback_{}", txn_id),
    )?;
    Ok(())
}
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
            execute_evolution_rollback(conn, chat_root, &txn_id, reason)?;
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod lib_tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;

    static EVO_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn strip_think_blocks_after_closing_tag() {
        let s = "<think>\nhidden\n</think>\nvisible reply";
        assert_eq!(strip_think_blocks(s), "visible reply");
    }

    #[test]
    fn strip_think_blocks_plain_text_unchanged() {
        let s = "no think tags here";
        assert_eq!(strip_think_blocks(s), s);
    }

    #[test]
    fn strip_think_blocks_reasoning_tag() {
        let s = "<reasoning>x</reasoning>\nhello";
        assert_eq!(strip_think_blocks(s), "hello");
    }

    #[test]
    fn evolution_message_constructors() {
        let u = EvolutionMessage::user("u");
        assert_eq!(u.role, "user");
        assert_eq!(u.content.as_deref(), Some("u"));
        let sy = EvolutionMessage::system("s");
        assert_eq!(sy.role, "system");
    }

    #[test]
    fn evolution_mode_capability_flags() {
        assert!(EvolutionMode::All.prompts_enabled());
        assert!(EvolutionMode::All.memory_enabled());
        assert!(EvolutionMode::All.skills_enabled());
        assert!(EvolutionMode::PromptsOnly.prompts_enabled());
        assert!(!EvolutionMode::PromptsOnly.memory_enabled());
        assert!(!EvolutionMode::MemoryOnly.prompts_enabled());
        assert!(EvolutionMode::MemoryOnly.memory_enabled());
        assert!(EvolutionMode::Disabled.is_disabled());
    }

    #[test]
    fn evolution_run_result_txn_id() {
        assert_eq!(
            EvolutionRunResult::Completed(Some("t1".into())).txn_id(),
            Some("t1")
        );
        assert_eq!(EvolutionRunResult::SkippedBusy.txn_id(), None);
    }

    #[test]
    fn gatekeeper_l2_size_bounds() {
        assert!(gatekeeper_l2_size(5, 3, 1));
        assert!(!gatekeeper_l2_size(6, 0, 0));
        assert!(!gatekeeper_l2_size(0, 4, 0));
        assert!(!gatekeeper_l2_size(0, 0, 2));
    }

    #[test]
    fn gatekeeper_l3_rejects_secret_pattern() {
        assert!(gatekeeper_l3_content("safe text").is_ok());
        assert!(gatekeeper_l3_content("has api_key in body").is_err());
    }

    #[test]
    fn gatekeeper_l1_path_allows_prompts_under_chat_root() {
        let root = Path::new("/home/u/.skilllite/chat");
        let target = root.join("prompts/rules.json");
        assert!(gatekeeper_l1_path(root, &target, None));
        let bad = Path::new("/etc/passwd");
        assert!(!gatekeeper_l1_path(root, bad, None));
    }

    #[test]
    fn try_start_evolution_is_exclusive() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        finish_evolution();
        assert!(try_start_evolution());
        assert!(!try_start_evolution());
        finish_evolution();
    }

    #[test]
    fn evolution_thresholds_default_nonzero_cooldown() {
        let t = EvolutionThresholds::default();
        assert!(t.cooldown_hours > 0.0);
        assert!(t.recent_days > 0);
    }
}
