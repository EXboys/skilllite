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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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

fn scope_has_work(scope: &EvolutionScope) -> bool {
    scope.prompts || scope.memory || scope.skills
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ProposalSource {
    Active,
    Passive,
}

impl ProposalSource {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Passive => "passive",
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum ProposalRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl ProposalRiskLevel {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }

    fn discount_factor(&self) -> f32 {
        match self {
            Self::Low => 1.0,
            Self::Medium => 0.8,
            Self::High => 0.55,
            Self::Critical => 0.3,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvolutionProposal {
    pub proposal_id: String,
    pub source: ProposalSource,
    pub scope: EvolutionScope,
    pub risk_level: ProposalRiskLevel,
    pub expected_gain: f32,
    pub effort: f32,
    pub roi_score: f32,
    pub dedupe_key: String,
    pub acceptance_criteria: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct EvolutionCoordinatorConfig {
    policy_runtime_enabled: bool,
    shadow_mode: bool,
    auto_execute_low_risk: bool,
    deny_critical: bool,
    risk_budget: EvolutionRiskBudget,
}

#[derive(Debug, Clone, Copy)]
struct EvolutionRiskBudget {
    low_per_day: i64,
    medium_per_day: i64,
    high_per_day: i64,
    critical_per_day: i64,
}

impl EvolutionRiskBudget {
    fn from_env() -> Self {
        Self {
            low_per_day: env_i64(evo_keys::SKILLLITE_EVO_RISK_BUDGET_LOW_PER_DAY, 5),
            medium_per_day: env_i64(evo_keys::SKILLLITE_EVO_RISK_BUDGET_MEDIUM_PER_DAY, 0),
            high_per_day: env_i64(evo_keys::SKILLLITE_EVO_RISK_BUDGET_HIGH_PER_DAY, 0),
            critical_per_day: env_i64(evo_keys::SKILLLITE_EVO_RISK_BUDGET_CRITICAL_PER_DAY, 0),
        }
    }

    fn limit_for(&self, risk: ProposalRiskLevel) -> i64 {
        match risk {
            ProposalRiskLevel::Low => self.low_per_day,
            ProposalRiskLevel::Medium => self.medium_per_day,
            ProposalRiskLevel::High => self.high_per_day,
            ProposalRiskLevel::Critical => self.critical_per_day,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolicyAction {
    Allow,
    Ask,
    Deny,
}

impl PolicyAction {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Ask => "ask",
            Self::Deny => "deny",
        }
    }
}

#[derive(Debug, Clone)]
struct PolicyRuntimeDecision {
    action: PolicyAction,
    shadow_mode_applied: bool,
    reasons: Vec<String>,
}

impl EvolutionCoordinatorConfig {
    fn from_env() -> Self {
        Self {
            policy_runtime_enabled: env_bool(evo_keys::SKILLLITE_EVO_POLICY_RUNTIME_ENABLED, true),
            shadow_mode: env_bool(evo_keys::SKILLLITE_EVO_SHADOW_MODE, true),
            auto_execute_low_risk: env_bool(evo_keys::SKILLLITE_EVO_AUTO_EXECUTE_LOW_RISK, false),
            deny_critical: env_bool(evo_keys::SKILLLITE_EVO_DENY_CRITICAL, true),
            risk_budget: EvolutionRiskBudget::from_env(),
        }
    }
}

enum CoordinatorDecision {
    NoCandidate,
    Shadow(EvolutionProposal),
    Queued(EvolutionProposal),
    Denied(EvolutionProposal),
    Execute(EvolutionProposal),
}

static EVOLUTION_COORDINATOR_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

fn try_start_coordinator() -> bool {
    EVOLUTION_COORDINATOR_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

fn finish_coordinator() {
    EVOLUTION_COORDINATOR_IN_PROGRESS.store(false, Ordering::SeqCst);
}

fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key).ok().as_deref().map(str::trim) {
        Some("1") | Some("true") | Some("TRUE") | Some("yes") | Some("on") => true,
        Some("0") | Some("false") | Some("FALSE") | Some("no") | Some("off") => false,
        Some(_) => default,
        None => default,
    }
}

fn env_i64(key: &str, default: i64) -> i64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(default)
}

fn count_daily_executions_by_risk(conn: &Connection, risk: ProposalRiskLevel) -> Result<i64> {
    let count = conn.query_row(
        "SELECT COUNT(*) FROM evolution_backlog
         WHERE date(updated_at) = date('now')
           AND risk_level = ?1
           AND status IN ('executing', 'executed')",
        params![risk.as_str()],
        |row| row.get(0),
    )?;
    Ok(count)
}

fn summarize_policy_runtime(decision: &PolicyRuntimeDecision) -> String {
    format!(
        "Policy runtime action={} ({})",
        decision.action.as_str(),
        decision.reasons.join(" -> ")
    )
}

fn evaluate_policy_runtime(
    conn: &Connection,
    proposal: &EvolutionProposal,
    config: EvolutionCoordinatorConfig,
) -> Result<PolicyRuntimeDecision> {
    let mut reasons = Vec::new();
    reasons.push(format!(
        "proposal risk={} roi={:.2}",
        proposal.risk_level.as_str(),
        proposal.roi_score
    ));

    if config.shadow_mode {
        reasons.push("shadow mode enabled".to_string());
        return Ok(PolicyRuntimeDecision {
            action: PolicyAction::Ask,
            shadow_mode_applied: true,
            reasons,
        });
    }

    if proposal.risk_level == ProposalRiskLevel::Critical && config.deny_critical {
        reasons.push("critical risk is denied by policy".to_string());
        return Ok(PolicyRuntimeDecision {
            action: PolicyAction::Deny,
            shadow_mode_applied: false,
            reasons,
        });
    }

    let daily_limit = config.risk_budget.limit_for(proposal.risk_level);
    let consumed = count_daily_executions_by_risk(conn, proposal.risk_level)?;
    reasons.push(format!(
        "daily budget {}/{} for {}",
        consumed,
        daily_limit,
        proposal.risk_level.as_str()
    ));
    if daily_limit <= 0 {
        reasons.push("auto budget disabled for this risk tier".to_string());
        return Ok(PolicyRuntimeDecision {
            action: PolicyAction::Ask,
            shadow_mode_applied: false,
            reasons,
        });
    }
    if consumed >= daily_limit {
        reasons.push("daily budget exhausted".to_string());
        return Ok(PolicyRuntimeDecision {
            action: PolicyAction::Ask,
            shadow_mode_applied: false,
            reasons,
        });
    }

    if proposal.risk_level == ProposalRiskLevel::Low && config.auto_execute_low_risk {
        reasons.push("low-risk auto execution enabled".to_string());
        return Ok(PolicyRuntimeDecision {
            action: PolicyAction::Allow,
            shadow_mode_applied: false,
            reasons,
        });
    }

    reasons.push("risk tier requires manual confirmation".to_string());
    Ok(PolicyRuntimeDecision {
        action: PolicyAction::Ask,
        shadow_mode_applied: false,
        reasons,
    })
}

fn compute_roi_score(expected_gain: f32, effort: f32, risk: ProposalRiskLevel) -> f32 {
    let safe_effort = effort.max(0.1);
    (expected_gain / safe_effort) * risk.discount_factor()
}

fn build_dedupe_key(source: ProposalSource, scope: &EvolutionScope) -> String {
    format!(
        "{}:{}:{}:{}:{:?}",
        source.as_str(),
        u8::from(scope.prompts),
        u8::from(scope.memory),
        u8::from(scope.skills),
        scope.skill_action
    )
}

fn build_proposal(
    source: ProposalSource,
    scope: EvolutionScope,
    risk_level: ProposalRiskLevel,
    expected_gain: f32,
    effort: f32,
    acceptance_criteria: Vec<String>,
) -> EvolutionProposal {
    let roi_score = compute_roi_score(expected_gain, effort, risk_level);
    let proposal_id = format!(
        "proposal_{}",
        chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f")
    );
    let dedupe_key = build_dedupe_key(source, &scope);
    EvolutionProposal {
        proposal_id,
        source,
        scope,
        risk_level,
        expected_gain,
        effort,
        roi_score,
        dedupe_key,
        acceptance_criteria,
    }
}

fn collect_active_scope(conn: &Connection, mode: EvolutionMode) -> Result<EvolutionScope> {
    if mode.is_disabled() {
        return Ok(EvolutionScope::default());
    }
    let threshold: i64 = std::env::var(evo_keys::SKILLLITE_EVOLUTION_DECISION_THRESHOLD)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let stable_successes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM decisions
             WHERE evolved = 0 AND task_completed = 1 AND failed_tools = 0 AND total_tools >= 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    if stable_successes < threshold {
        return Ok(EvolutionScope::default());
    }
    let mut stmt = conn.prepare(
        "SELECT id FROM decisions
         WHERE evolved = 0 AND task_completed = 1 AND failed_tools = 0
         ORDER BY ts DESC LIMIT 100",
    )?;
    let decision_ids: Vec<i64> = stmt
        .query_map([], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    let mut scope = EvolutionScope {
        decision_ids,
        ..Default::default()
    };
    if mode.memory_enabled() {
        scope.memory = true;
    } else if mode.prompts_enabled() {
        scope.prompts = true;
    } else if mode.skills_enabled() {
        scope.skills = true;
        scope.skill_action = SkillAction::Refine;
    }
    Ok(scope)
}

fn build_evolution_proposals(
    conn: &Connection,
    mode: EvolutionMode,
    force: bool,
) -> Result<Vec<EvolutionProposal>> {
    let mut proposals = Vec::new();

    let passive_scope = should_evolve_impl(conn, mode.clone(), force)?;
    if scope_has_work(&passive_scope) {
        proposals.push(build_proposal(
            ProposalSource::Passive,
            passive_scope,
            ProposalRiskLevel::Medium,
            0.85,
            2.0,
            vec![
                "No regression in first_success_rate over next 3 daily windows.".to_string(),
                "No rise in user_correction_rate over next 3 daily windows.".to_string(),
            ],
        ));
    }

    let active_scope = collect_active_scope(conn, mode)?;
    if scope_has_work(&active_scope) {
        proposals.push(build_proposal(
            ProposalSource::Active,
            active_scope,
            ProposalRiskLevel::Low,
            0.45,
            1.0,
            vec![
                "At least one measurable signal improves after execution.".to_string(),
                "No security or quality gate regressions introduced.".to_string(),
            ],
        ));
    }

    Ok(proposals)
}

fn upsert_backlog_proposal(
    conn: &Connection,
    proposal: &EvolutionProposal,
    status: &str,
    note: &str,
) -> Result<()> {
    let scope_json = serde_json::to_string(&proposal.scope)?;
    let acceptance_criteria = serde_json::to_string(&proposal.acceptance_criteria)?;
    conn.execute(
        "INSERT OR IGNORE INTO evolution_backlog
         (proposal_id, source, dedupe_key, scope_json, risk_level, roi_score, expected_gain, effort, acceptance_criteria, status, note)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            proposal.proposal_id,
            proposal.source.as_str(),
            proposal.dedupe_key,
            scope_json,
            proposal.risk_level.as_str(),
            proposal.roi_score as f64,
            proposal.expected_gain as f64,
            proposal.effort as f64,
            acceptance_criteria,
            status,
            note,
        ],
    )?;
    conn.execute(
        "UPDATE evolution_backlog
         SET roi_score = ?1,
             expected_gain = ?2,
             effort = ?3,
             updated_at = datetime('now'),
             note = ?4
         WHERE dedupe_key = ?5 AND status != 'executed'",
        params![
            proposal.roi_score as f64,
            proposal.expected_gain as f64,
            proposal.effort as f64,
            note,
            proposal.dedupe_key,
        ],
    )?;
    Ok(())
}

fn set_backlog_status(
    conn: &Connection,
    proposal_id: &str,
    status: &str,
    acceptance_status: &str,
    note: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE evolution_backlog
         SET status = ?1, acceptance_status = ?2, note = ?3, updated_at = datetime('now')
         WHERE proposal_id = ?4",
        params![status, acceptance_status, note, proposal_id],
    )?;
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct AcceptanceThresholds {
    window_days: i64,
    min_success_rate: f64,
    max_correction_rate: f64,
    max_rollback_rate: f64,
}

impl Default for AcceptanceThresholds {
    fn default() -> Self {
        Self {
            window_days: 3,
            min_success_rate: 0.70,
            max_correction_rate: 0.20,
            max_rollback_rate: 0.20,
        }
    }
}

impl AcceptanceThresholds {
    fn from_env() -> Self {
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
        let base = Self::default();
        Self {
            window_days: parse_i64(
                evo_keys::SKILLLITE_EVO_ACCEPTANCE_WINDOW_DAYS,
                base.window_days,
            )
            .max(1),
            min_success_rate: parse_f64(
                evo_keys::SKILLLITE_EVO_ACCEPTANCE_MIN_SUCCESS_RATE,
                base.min_success_rate,
            )
            .clamp(0.0, 1.0),
            max_correction_rate: parse_f64(
                evo_keys::SKILLLITE_EVO_ACCEPTANCE_MAX_CORRECTION_RATE,
                base.max_correction_rate,
            )
            .clamp(0.0, 1.0),
            max_rollback_rate: parse_f64(
                evo_keys::SKILLLITE_EVO_ACCEPTANCE_MAX_ROLLBACK_RATE,
                base.max_rollback_rate,
            )
            .clamp(0.0, 1.0),
        }
    }
}

fn auto_link_acceptance_status(conn: &Connection, proposal_id: &str) -> Result<()> {
    let thresholds = AcceptanceThresholds::from_env();
    let updated_at: String = conn.query_row(
        "SELECT updated_at FROM evolution_backlog WHERE proposal_id = ?1",
        params![proposal_id],
        |row| row.get(0),
    )?;

    let (window_days, avg_success_rate, avg_correction_rate): (i64, f64, f64) = conn.query_row(
        "SELECT
            COUNT(*),
            COALESCE(AVG(first_success_rate), 0.0),
            COALESCE(AVG(user_correction_rate), 0.0)
         FROM evolution_metrics
         WHERE date >= date(?1)
           AND date < date(?1, ?2)",
        params![updated_at, format!("+{} days", thresholds.window_days)],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;

    if window_days < thresholds.window_days {
        let note = format!(
            "Awaiting acceptance window: collected {}/{} daily metrics",
            window_days, thresholds.window_days
        );
        set_backlog_status(conn, proposal_id, "executed", "pending_validation", &note)?;
        return Ok(());
    }

    let (run_count, rollback_count): (i64, i64) = conn.query_row(
        "SELECT
            COUNT(CASE WHEN type LIKE 'evolution_run%' THEN 1 END),
            COUNT(CASE WHEN type LIKE 'auto_rollback%' THEN 1 END)
         FROM evolution_log
         WHERE date(ts) >= date(?1)
           AND date(ts) < date(?1, ?2)",
        params![updated_at, format!("+{} days", thresholds.window_days)],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let rollback_rate = if run_count > 0 {
        rollback_count as f64 / run_count as f64
    } else {
        0.0
    };

    let met = avg_success_rate >= thresholds.min_success_rate
        && avg_correction_rate <= thresholds.max_correction_rate
        && rollback_rate <= thresholds.max_rollback_rate;
    let acceptance_status = if met { "met" } else { "not_met" };
    let note = format!(
        "Acceptance window({}d): success={:.2}, correction={:.2}, rollback={:.2} ({}/{}) => {}",
        thresholds.window_days,
        avg_success_rate,
        avg_correction_rate,
        rollback_rate,
        rollback_count,
        run_count,
        acceptance_status
    );
    set_backlog_status(conn, proposal_id, "executed", acceptance_status, &note)?;
    Ok(())
}

fn coordinate_proposals(
    conn: &Connection,
    proposals: Vec<EvolutionProposal>,
    force: bool,
) -> Result<CoordinatorDecision> {
    coordinate_proposals_with_config(
        conn,
        proposals,
        force,
        EvolutionCoordinatorConfig::from_env(),
    )
}

fn coordinate_proposals_with_config(
    conn: &Connection,
    mut proposals: Vec<EvolutionProposal>,
    force: bool,
    config: EvolutionCoordinatorConfig,
) -> Result<CoordinatorDecision> {
    if proposals.is_empty() {
        return Ok(CoordinatorDecision::NoCandidate);
    }
    if !try_start_coordinator() {
        tracing::warn!("Evolution coordinator busy; skipping this round");
        return Ok(CoordinatorDecision::NoCandidate);
    }
    let result = (|| -> Result<CoordinatorDecision> {
        for proposal in &proposals {
            upsert_backlog_proposal(conn, proposal, "queued", "Proposal collected")?;
        }
        proposals.sort_by(|a, b| b.roi_score.total_cmp(&a.roi_score));
        let Some(selected) = proposals.into_iter().next() else {
            return Ok(CoordinatorDecision::NoCandidate);
        };
        if force {
            set_backlog_status(
                conn,
                &selected.proposal_id,
                "executing",
                "pending",
                "Forced run bypassed coordinator execution gate",
            )?;
            return Ok(CoordinatorDecision::Execute(selected));
        }
        if !config.policy_runtime_enabled {
            if config.shadow_mode {
                set_backlog_status(
                    conn,
                    &selected.proposal_id,
                    "shadow_approved",
                    "pending",
                    "Shadow mode enabled: proposal queued only",
                )?;
                return Ok(CoordinatorDecision::Shadow(selected));
            }
            if config.auto_execute_low_risk && selected.risk_level == ProposalRiskLevel::Low {
                set_backlog_status(
                    conn,
                    &selected.proposal_id,
                    "executing",
                    "pending",
                    "Auto execution allowed for low-risk proposal",
                )?;
                return Ok(CoordinatorDecision::Execute(selected));
            }
            set_backlog_status(
                conn,
                &selected.proposal_id,
                "queued",
                "pending",
                "Waiting for manual or policy-based execution",
            )?;
            return Ok(CoordinatorDecision::Queued(selected));
        }

        let policy = evaluate_policy_runtime(conn, &selected, config)?;
        let note = summarize_policy_runtime(&policy);
        match policy.action {
            PolicyAction::Allow => {
                set_backlog_status(conn, &selected.proposal_id, "executing", "pending", &note)?;
                Ok(CoordinatorDecision::Execute(selected))
            }
            PolicyAction::Ask => {
                if policy.shadow_mode_applied {
                    set_backlog_status(
                        conn,
                        &selected.proposal_id,
                        "shadow_approved",
                        "pending",
                        &note,
                    )?;
                    Ok(CoordinatorDecision::Shadow(selected))
                } else {
                    set_backlog_status(conn, &selected.proposal_id, "queued", "pending", &note)?;
                    Ok(CoordinatorDecision::Queued(selected))
                }
            }
            PolicyAction::Deny => {
                set_backlog_status(
                    conn,
                    &selected.proposal_id,
                    "policy_denied",
                    "rejected",
                    &note,
                )?;
                Ok(CoordinatorDecision::Denied(selected))
            }
        }
    })();
    finish_coordinator();
    result
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

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            if let Some(parent) = dst_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn create_extended_snapshot(
    chat_root: &Path,
    skills_root: Option<&Path>,
    txn_id: &str,
    include_prompts: bool,
    include_memory: bool,
    include_skills: bool,
) -> Result<Vec<String>> {
    let mut backed_up = Vec::new();
    if include_prompts {
        backed_up.extend(create_snapshot(
            chat_root,
            txn_id,
            &[
                "rules.json",
                "examples.json",
                "planning.md",
                "execution.md",
                "system.md",
            ],
        )?);
    } else {
        let snap_dir = versions_dir(chat_root).join(txn_id);
        std::fs::create_dir_all(&snap_dir)?;
    }

    let snap_dir = versions_dir(chat_root).join(txn_id);
    if include_memory {
        let memory_src = chat_root
            .join("memory")
            .join("evolution")
            .join("knowledge.md");
        if memory_src.exists() {
            let memory_dst = snap_dir.join("memory").join("knowledge.md");
            if let Some(parent) = memory_dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(memory_src, memory_dst)?;
            backed_up.push("memory/evolution/knowledge.md".to_string());
        }
    }

    if include_skills {
        if let Some(sr) = skills_root {
            let evolved_src = sr.join("_evolved");
            if evolved_src.exists() {
                let evolved_dst = snap_dir.join("skills").join("_evolved");
                copy_dir_recursive(&evolved_src, &evolved_dst)?;
                backed_up.push("skills/_evolved".to_string());
            }
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
        if entry.file_type()?.is_dir() {
            continue;
        }
        let dst = prompts.join(entry.file_name());
        std::fs::copy(entry.path(), &dst)?;
    }
    tracing::info!("Restored snapshot {}", txn_id);
    Ok(())
}

fn restore_extended_snapshot(
    chat_root: &Path,
    skills_root: Option<&Path>,
    txn_id: &str,
) -> Result<()> {
    restore_snapshot(chat_root, txn_id)?;
    let snap_dir = versions_dir(chat_root).join(txn_id);

    let memory_src = snap_dir.join("memory").join("knowledge.md");
    if memory_src.exists() {
        let memory_dst = chat_root
            .join("memory")
            .join("evolution")
            .join("knowledge.md");
        if let Some(parent) = memory_dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(memory_src, memory_dst)?;
    }

    let skills_src = snap_dir.join("skills").join("_evolved");
    if skills_src.exists() {
        if let Some(sr) = skills_root {
            let skills_dst = sr.join("_evolved");
            if skills_dst.exists() {
                std::fs::remove_dir_all(&skills_dst)?;
            }
            copy_dir_recursive(&skills_src, &skills_dst)?;
        }
    }
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
    let proposals = build_evolution_proposals(&conn, EvolutionMode::from_env(), force)?;
    let decision = coordinate_proposals(&conn, proposals, force)?;
    let (scope, proposal) = match decision {
        CoordinatorDecision::NoCandidate => return Ok(EvolutionRunResult::NoScope),
        CoordinatorDecision::Shadow(p) => {
            let reason = format!(
                "Proposal {} ({}) accepted in shadow mode; execution deferred",
                p.proposal_id,
                p.source.as_str()
            );
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_proposal",
                &p.proposal_id,
                &reason,
                "",
            );
            return Ok(EvolutionRunResult::Completed(None));
        }
        CoordinatorDecision::Queued(p) => {
            let reason = format!(
                "Proposal {} ({}) queued; waiting execution gate",
                p.proposal_id,
                p.source.as_str()
            );
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_proposal",
                &p.proposal_id,
                &reason,
                "",
            );
            return Ok(EvolutionRunResult::Completed(None));
        }
        CoordinatorDecision::Denied(p) => {
            let reason = format!(
                "Proposal {} ({}) denied by policy runtime",
                p.proposal_id,
                p.source.as_str()
            );
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "evolution_proposal_denied",
                &p.proposal_id,
                &reason,
                "",
            );
            return Ok(EvolutionRunResult::Completed(None));
        }
        CoordinatorDecision::Execute(p) => (p.scope.clone(), p),
    };

    let txn_id = format!("evo_{}", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
    tracing::info!(
        "Starting evolution txn={} proposal={} source={} (prompts={}, memory={}, skills={})",
        txn_id,
        proposal.proposal_id,
        proposal.source.as_str(),
        scope.prompts,
        scope.memory,
        scope.skills
    );
    let snapshot_files = create_extended_snapshot(
        chat_root,
        skills_root,
        &txn_id,
        scope.prompts,
        scope.memory,
        scope.skills,
    )?;

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
        let auto_rolled_back = check_auto_rollback(&conn, chat_root, skills_root)?;
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
            let _ = set_backlog_status(
                &conn,
                &proposal.proposal_id,
                "executed",
                "not_met",
                "Executed with no material changes",
            );
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
        let _ = set_backlog_status(
            &conn,
            &proposal.proposal_id,
            "executed",
            "pending_validation",
            "Execution completed; awaiting acceptance metrics window",
        );
        if let Err(e) = auto_link_acceptance_status(&conn, &proposal.proposal_id) {
            tracing::warn!(
                "Failed to auto-link acceptance status for proposal {}: {}",
                proposal.proposal_id,
                e
            );
        }

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
    skills_root: Option<&Path>,
    txn_id: &str,
    reason: &str,
) -> Result<()> {
    tracing::warn!("Evolution rollback executed: {} (txn={})", reason, txn_id);
    restore_extended_snapshot(chat_root, skills_root, txn_id)?;

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
pub fn check_auto_rollback(
    conn: &Connection,
    chat_root: &Path,
    skills_root: Option<&Path>,
) -> Result<bool> {
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
            execute_evolution_rollback(conn, chat_root, skills_root, &txn_id, reason)?;
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

    #[test]
    fn roi_score_penalizes_risk() {
        let low = compute_roi_score(1.0, 1.0, ProposalRiskLevel::Low);
        let high = compute_roi_score(1.0, 1.0, ProposalRiskLevel::High);
        assert!(low > high);
    }

    #[test]
    fn coordinator_shadow_mode_queues_without_execution() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let scope = EvolutionScope {
            memory: true,
            ..Default::default()
        };
        let proposal = build_proposal(
            ProposalSource::Active,
            scope,
            ProposalRiskLevel::Low,
            0.5,
            1.0,
            vec!["metric should improve".to_string()],
        );
        let decision = coordinate_proposals_with_config(
            &conn,
            vec![proposal],
            false,
            EvolutionCoordinatorConfig {
                policy_runtime_enabled: true,
                shadow_mode: true,
                auto_execute_low_risk: true,
                deny_critical: true,
                risk_budget: EvolutionRiskBudget {
                    low_per_day: 5,
                    medium_per_day: 0,
                    high_per_day: 0,
                    critical_per_day: 0,
                },
            },
        )
        .expect("coordinate");
        assert!(matches!(decision, CoordinatorDecision::Shadow(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn coordinator_auto_executes_low_risk_when_enabled() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let scope = EvolutionScope {
            memory: true,
            ..Default::default()
        };
        let proposal = build_proposal(
            ProposalSource::Active,
            scope,
            ProposalRiskLevel::Low,
            0.5,
            1.0,
            vec!["metric should improve".to_string()],
        );
        let decision = coordinate_proposals_with_config(
            &conn,
            vec![proposal],
            false,
            EvolutionCoordinatorConfig {
                policy_runtime_enabled: true,
                shadow_mode: false,
                auto_execute_low_risk: true,
                deny_critical: true,
                risk_budget: EvolutionRiskBudget {
                    low_per_day: 5,
                    medium_per_day: 0,
                    high_per_day: 0,
                    critical_per_day: 0,
                },
            },
        )
        .expect("coordinate");
        assert!(matches!(decision, CoordinatorDecision::Execute(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn coordinator_queues_when_low_risk_budget_exhausted() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let scope = EvolutionScope {
            memory: true,
            ..Default::default()
        };
        let proposal = build_proposal(
            ProposalSource::Active,
            scope,
            ProposalRiskLevel::Low,
            0.5,
            1.0,
            vec!["metric should improve".to_string()],
        );

        conn.execute(
            "INSERT INTO evolution_backlog
             (proposal_id, source, dedupe_key, scope_json, risk_level, roi_score, expected_gain, effort, acceptance_criteria, status, note)
             VALUES (?1, 'active', ?2, '{}', 'low', 0.1, 0.1, 1.0, '[]', 'executed', 'seed')",
            rusqlite::params![
                "seed_proposal",
                format!("seed_{}", uuid::Uuid::new_v4()),
            ],
        )
        .expect("insert seed");

        let decision = coordinate_proposals_with_config(
            &conn,
            vec![proposal],
            false,
            EvolutionCoordinatorConfig {
                policy_runtime_enabled: true,
                shadow_mode: false,
                auto_execute_low_risk: true,
                deny_critical: true,
                risk_budget: EvolutionRiskBudget {
                    low_per_day: 1,
                    medium_per_day: 0,
                    high_per_day: 0,
                    critical_per_day: 0,
                },
            },
        )
        .expect("coordinate");
        assert!(matches!(decision, CoordinatorDecision::Queued(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn coordinator_denies_critical_when_policy_enabled() {
        let _g = EVO_LOCK.lock().expect("evo lock");
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        let scope = EvolutionScope {
            skills: true,
            skill_action: SkillAction::Generate,
            ..Default::default()
        };
        let proposal = build_proposal(
            ProposalSource::Passive,
            scope,
            ProposalRiskLevel::Critical,
            0.9,
            3.0,
            vec!["no regressions".to_string()],
        );
        let decision = coordinate_proposals_with_config(
            &conn,
            vec![proposal],
            false,
            EvolutionCoordinatorConfig {
                policy_runtime_enabled: true,
                shadow_mode: false,
                auto_execute_low_risk: true,
                deny_critical: true,
                risk_budget: EvolutionRiskBudget {
                    low_per_day: 5,
                    medium_per_day: 0,
                    high_per_day: 0,
                    critical_per_day: 1,
                },
            },
        )
        .expect("coordinate");
        assert!(matches!(decision, CoordinatorDecision::Denied(_)));
        let _ = std::fs::remove_dir_all(&root);
    }

    fn seed_backlog_row(conn: &Connection, proposal_id: &str, updated_at: &str) {
        conn.execute(
            "INSERT INTO evolution_backlog
             (proposal_id, source, dedupe_key, scope_json, risk_level, roi_score, expected_gain, effort, acceptance_criteria, status, acceptance_status, note, updated_at)
             VALUES (?1, 'active', ?2, '{}', 'low', 0.5, 0.5, 1.0, '[]', 'executed', 'pending_validation', 'seed', ?3)",
            rusqlite::params![proposal_id, format!("dedupe_{}", proposal_id), updated_at],
        )
        .expect("insert backlog row");
    }

    #[test]
    fn auto_link_acceptance_stays_pending_without_full_window() {
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        seed_backlog_row(&conn, "p_pending", "2026-04-01 00:00:00");

        conn.execute(
            "INSERT INTO evolution_metrics (date, first_success_rate, avg_replans, avg_tool_calls, user_correction_rate, egl)
             VALUES ('2026-04-01', 0.90, 0.1, 1.0, 0.05, 0.0)",
            [],
        )
        .expect("insert metric");
        conn.execute(
            "INSERT INTO evolution_log (ts, type, target_id, reason, version)
             VALUES ('2026-04-01T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-1')",
            [],
        )
        .expect("insert run");

        auto_link_acceptance_status(&conn, "p_pending").expect("auto link");
        let (status, note): (String, String) = conn
            .query_row(
                "SELECT acceptance_status, note FROM evolution_backlog WHERE proposal_id = 'p_pending'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read backlog");
        assert_eq!(status, "pending_validation");
        assert!(note.contains("Awaiting acceptance window"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn auto_link_acceptance_marks_met_on_healthy_window() {
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        seed_backlog_row(&conn, "p_met", "2026-04-01 00:00:00");

        conn.execute_batch(
            "INSERT INTO evolution_metrics (date, first_success_rate, avg_replans, avg_tool_calls, user_correction_rate, egl)
             VALUES
             ('2026-04-01', 0.82, 0.1, 1.0, 0.08, 0.0),
             ('2026-04-02', 0.85, 0.1, 1.0, 0.10, 0.0),
             ('2026-04-03', 0.80, 0.1, 1.0, 0.12, 0.0);
             INSERT INTO evolution_log (ts, type, target_id, reason, version) VALUES
             ('2026-04-01T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-1'),
             ('2026-04-02T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-2'),
             ('2026-04-03T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-3');",
        )
        .expect("seed metrics and runs");

        auto_link_acceptance_status(&conn, "p_met").expect("auto link");
        let status: String = conn
            .query_row(
                "SELECT acceptance_status FROM evolution_backlog WHERE proposal_id = 'p_met'",
                [],
                |row| row.get(0),
            )
            .expect("read status");
        assert_eq!(status, "met");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn auto_link_acceptance_marks_not_met_when_rollback_rate_high() {
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let conn = feedback::open_evolution_db(&root).expect("open db");
        seed_backlog_row(&conn, "p_not_met", "2026-04-01 00:00:00");

        conn.execute_batch(
            "INSERT INTO evolution_metrics (date, first_success_rate, avg_replans, avg_tool_calls, user_correction_rate, egl)
             VALUES
             ('2026-04-01', 0.85, 0.1, 1.0, 0.10, 0.0),
             ('2026-04-02', 0.88, 0.1, 1.0, 0.10, 0.0),
             ('2026-04-03', 0.87, 0.1, 1.0, 0.10, 0.0);
             INSERT INTO evolution_log (ts, type, target_id, reason, version) VALUES
             ('2026-04-01T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-1'),
             ('2026-04-02T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-2'),
             ('2026-04-03T08:00:00Z', 'evolution_run', 'run', 'seed', 'txn-3'),
             ('2026-04-02T09:00:00Z', 'auto_rollback', 'txn-2', 'decline', 'rollback_txn-2');",
        )
        .expect("seed metrics and rollback");

        auto_link_acceptance_status(&conn, "p_not_met").expect("auto link");
        let status: String = conn
            .query_row(
                "SELECT acceptance_status FROM evolution_backlog WHERE proposal_id = 'p_not_met'",
                [],
                |row| row.get(0),
            )
            .expect("read status");
        assert_eq!(status, "not_met");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn acceptance_thresholds_read_from_env_and_clamped() {
        std::env::set_var("SKILLLITE_EVO_ACCEPTANCE_WINDOW_DAYS", "0");
        std::env::set_var("SKILLLITE_EVO_ACCEPTANCE_MIN_SUCCESS_RATE", "1.2");
        std::env::set_var("SKILLLITE_EVO_ACCEPTANCE_MAX_CORRECTION_RATE", "-0.1");
        std::env::set_var("SKILLLITE_EVO_ACCEPTANCE_MAX_ROLLBACK_RATE", "0.35");
        let t = AcceptanceThresholds::from_env();
        assert_eq!(t.window_days, 1);
        assert!((t.min_success_rate - 1.0).abs() < 1e-9);
        assert!((t.max_correction_rate - 0.0).abs() < 1e-9);
        assert!((t.max_rollback_rate - 0.35).abs() < 1e-9);
        std::env::remove_var("SKILLLITE_EVO_ACCEPTANCE_WINDOW_DAYS");
        std::env::remove_var("SKILLLITE_EVO_ACCEPTANCE_MIN_SUCCESS_RATE");
        std::env::remove_var("SKILLLITE_EVO_ACCEPTANCE_MAX_CORRECTION_RATE");
        std::env::remove_var("SKILLLITE_EVO_ACCEPTANCE_MAX_ROLLBACK_RATE");
    }

    #[test]
    fn extended_snapshot_restores_memory_and_skills() {
        let root =
            std::env::temp_dir().join(format!("skilllite-evo-test-{}", uuid::Uuid::new_v4()));
        let skills_root = root.join("skills_project");
        let prompts_dir = root.join("prompts");
        let memory_dir = root.join("memory").join("evolution");
        let evolved_dir = skills_root.join("_evolved").join("s1");
        std::fs::create_dir_all(&prompts_dir).expect("prompts");
        std::fs::create_dir_all(&memory_dir).expect("memory");
        std::fs::create_dir_all(&evolved_dir).expect("skills");
        std::fs::write(prompts_dir.join("rules.json"), b"before_rules").expect("rules");
        std::fs::write(memory_dir.join("knowledge.md"), b"before_memory").expect("memory");
        std::fs::write(evolved_dir.join("SKILL.md"), b"before_skill").expect("skill");

        let snap = create_extended_snapshot(&root, Some(&skills_root), "txn_x", true, true, true)
            .expect("snapshot");
        assert!(snap.iter().any(|f| f == "memory/evolution/knowledge.md"));
        assert!(snap.iter().any(|f| f == "skills/_evolved"));

        std::fs::write(prompts_dir.join("rules.json"), b"after_rules").expect("rules mutate");
        std::fs::write(memory_dir.join("knowledge.md"), b"after_memory").expect("memory mutate");
        std::fs::write(evolved_dir.join("SKILL.md"), b"after_skill").expect("skill mutate");

        restore_extended_snapshot(&root, Some(&skills_root), "txn_x").expect("restore");
        let rules = std::fs::read_to_string(prompts_dir.join("rules.json")).expect("rules read");
        let memory = std::fs::read_to_string(memory_dir.join("knowledge.md")).expect("memory read");
        let skill = std::fs::read_to_string(evolved_dir.join("SKILL.md")).expect("skill read");
        assert_eq!(rules, "before_rules");
        assert_eq!(memory, "before_memory");
        assert_eq!(skill, "before_skill");
        let _ = std::fs::remove_dir_all(&root);
    }
}
