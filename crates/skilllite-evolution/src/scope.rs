//! Evolution scope, proposals, backlog coordinator, and should_evolve.

use std::sync::atomic::{AtomicBool, Ordering};

use rusqlite::{params, Connection};
use skilllite_core::config::env_keys::evolution as evo_keys;

use crate::config::{EvolutionMode, EvolutionThresholds, SkillAction};
use crate::error::bail;
use crate::Result;

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
    pub(crate) fn as_str(&self) -> &'static str {
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
pub(crate) struct EvolutionCoordinatorConfig {
    pub(crate) policy_runtime_enabled: bool,
    pub(crate) auto_execute_low_risk: bool,
    pub(crate) deny_critical: bool,
    pub(crate) risk_budget: EvolutionRiskBudget,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct EvolutionRiskBudget {
    pub(crate) low_per_day: i64,
    pub(crate) medium_per_day: i64,
    pub(crate) high_per_day: i64,
    pub(crate) critical_per_day: i64,
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
    reasons: Vec<String>,
}

impl EvolutionCoordinatorConfig {
    fn from_env() -> Self {
        Self {
            policy_runtime_enabled: env_bool(evo_keys::SKILLLITE_EVO_POLICY_RUNTIME_ENABLED, true),
            auto_execute_low_risk: env_bool(evo_keys::SKILLLITE_EVO_AUTO_EXECUTE_LOW_RISK, true),
            deny_critical: env_bool(evo_keys::SKILLLITE_EVO_DENY_CRITICAL, true),
            risk_budget: EvolutionRiskBudget::from_env(),
        }
    }
}

pub(crate) enum CoordinatorDecision {
    NoCandidate,
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

    if proposal.risk_level == ProposalRiskLevel::Critical && config.deny_critical {
        reasons.push("critical risk is denied by policy".to_string());
        return Ok(PolicyRuntimeDecision {
            action: PolicyAction::Deny,
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
            reasons,
        });
    }
    if consumed >= daily_limit {
        reasons.push("daily budget exhausted".to_string());
        return Ok(PolicyRuntimeDecision {
            action: PolicyAction::Ask,
            reasons,
        });
    }

    if proposal.risk_level == ProposalRiskLevel::Low && config.auto_execute_low_risk {
        reasons.push("low-risk auto execution enabled".to_string());
        return Ok(PolicyRuntimeDecision {
            action: PolicyAction::Allow,
            reasons,
        });
    }

    reasons.push("risk tier requires manual confirmation".to_string());
    Ok(PolicyRuntimeDecision {
        action: PolicyAction::Ask,
        reasons,
    })
}

pub(crate) fn compute_roi_score(expected_gain: f32, effort: f32, risk: ProposalRiskLevel) -> f32 {
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

pub(crate) fn build_proposal(
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
    let min_stable: i64 = std::env::var(evo_keys::SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS)
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
    if stable_successes < min_stable {
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

pub(crate) fn build_evolution_proposals(
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

/// True when [`build_evolution_proposals`] would return a non-empty list (same env/mode semantics).
pub fn would_have_evolution_proposals(
    conn: &Connection,
    mode: EvolutionMode,
    force: bool,
) -> Result<bool> {
    Ok(!build_evolution_proposals(conn, mode, force)?.is_empty())
}

/// Stable English reason for audit / UI when [`build_evolution_proposals`] returns an empty list.
pub fn describe_empty_evolution_proposals(
    conn: &Connection,
    mode: &EvolutionMode,
    force: bool,
) -> Result<&'static str> {
    if mode.is_disabled() {
        return Ok("NoScope: evolution disabled (SKILLLITE_EVOLUTION)");
    }

    if !force {
        let today_evolutions: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM evolution_log
                 WHERE date(ts) = date('now') AND type = 'evolution_run'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        let max_per_day: i64 = std::env::var(evo_keys::SKILLLITE_MAX_EVOLUTIONS_PER_DAY)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(20);
        if today_evolutions >= max_per_day {
            return Ok("NoScope: daily evolution cap reached (SKILLLITE_MAX_EVOLUTIONS_PER_DAY)");
        }

        let thresholds = EvolutionThresholds::from_env();
        let last_evo_hours: f64 = conn
            .query_row(
                "SELECT COALESCE(
                    (julianday('now') - julianday(MAX(ts))) * 24,
                    999.0
                ) FROM evolution_log WHERE type = 'evolution_run'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(999.0);
        if last_evo_hours < thresholds.cooldown_hours {
            return Ok(
                "NoScope: cooldown active since last evolution_run (SKILLLITE_EVO_COOLDOWN_HOURS)",
            );
        }
    }

    Ok("NoScope: passive and active scopes idle (thresholds; SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS)")
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

fn latest_non_executed_proposal_id_by_dedupe(
    conn: &Connection,
    dedupe_key: &str,
) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT proposal_id
         FROM evolution_backlog
         WHERE dedupe_key = ?1 AND status != 'executed'
         ORDER BY updated_at DESC, proposal_id DESC
         LIMIT 1",
    )?;
    let row = stmt.query_row([dedupe_key], |row| row.get::<_, String>(0));
    match row {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Enqueue a user-authorized capability evolution proposal into backlog.
///
/// This is intentionally bounded to backlog insertion only. It does not bypass
/// coordinator policy runtime or force immediate execution.
pub fn enqueue_user_capability_evolution(
    conn: &Connection,
    tool_name: &str,
    outcome: &str,
    summary: &str,
) -> Result<String> {
    let outcome_norm = outcome.trim().to_ascii_lowercase();
    let risk_level = match outcome_norm.as_str() {
        "failure" => ProposalRiskLevel::Medium,
        "partial_success" => ProposalRiskLevel::Low,
        other => bail!(
            "unsupported capability outcome '{}'; expected partial_success or failure",
            other
        ),
    };
    let clean_tool = tool_name.trim();
    if clean_tool.is_empty() {
        bail!("tool_name cannot be empty for capability evolution authorization");
    }
    let clean_summary = summary.trim();
    let summary_preview: String = clean_summary.chars().take(160).collect();
    let proposal = EvolutionProposal {
        proposal_id: format!(
            "proposal_{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S%.3f")
        ),
        source: ProposalSource::Passive,
        scope: EvolutionScope {
            skills: true,
            skill_action: SkillAction::Generate,
            ..Default::default()
        },
        risk_level,
        expected_gain: if outcome_norm == "failure" {
            0.75
        } else {
            0.55
        },
        effort: if outcome_norm == "failure" { 1.8 } else { 1.2 },
        roi_score: compute_roi_score(
            if outcome_norm == "failure" {
                0.75
            } else {
                0.55
            },
            if outcome_norm == "failure" { 1.8 } else { 1.2 },
            risk_level,
        ),
        dedupe_key: format!(
            "user_capability:{}:{}",
            clean_tool.to_ascii_lowercase(),
            outcome_norm
        ),
        acceptance_criteria: vec![
            format!(
                "Capability gap for tool '{}' ({}) is reduced in follow-up runs.",
                clean_tool, outcome_norm
            ),
            "No regressions in safety gates and acceptance metrics over the next 3 daily windows."
                .to_string(),
        ],
    };
    let note = if summary_preview.is_empty() {
        format!(
            "User authorized capability evolution (tool={}, outcome={})",
            clean_tool, outcome_norm
        )
    } else {
        format!(
            "User authorized capability evolution (tool={}, outcome={}): {}",
            clean_tool, outcome_norm, summary_preview
        )
    };
    upsert_backlog_proposal(conn, &proposal, "queued", &note)?;
    Ok(
        latest_non_executed_proposal_id_by_dedupe(conn, &proposal.dedupe_key)?
            .unwrap_or(proposal.proposal_id),
    )
}

pub(crate) fn set_backlog_status(
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

fn parse_proposal_source(s: &str) -> Option<ProposalSource> {
    match s.trim().to_ascii_lowercase().as_str() {
        "active" => Some(ProposalSource::Active),
        "passive" => Some(ProposalSource::Passive),
        _ => None,
    }
}

fn parse_proposal_risk(s: &str) -> Option<ProposalRiskLevel> {
    match s.trim().to_ascii_lowercase().as_str() {
        "low" => Some(ProposalRiskLevel::Low),
        "medium" => Some(ProposalRiskLevel::Medium),
        "high" => Some(ProposalRiskLevel::High),
        "critical" => Some(ProposalRiskLevel::Critical),
        _ => None,
    }
}

pub(crate) fn load_backlog_proposal_by_id(
    conn: &Connection,
    proposal_id: &str,
) -> Result<Option<EvolutionProposal>> {
    let mut stmt = conn.prepare(
        "SELECT proposal_id, source, dedupe_key, scope_json, risk_level, roi_score, expected_gain, effort, acceptance_criteria
         FROM evolution_backlog
         WHERE proposal_id = ?1
         LIMIT 1",
    )?;
    let row = stmt.query_row([proposal_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, f32>(5)?,
            row.get::<_, f32>(6)?,
            row.get::<_, f32>(7)?,
            row.get::<_, String>(8)?,
        ))
    });
    let Ok((
        proposal_id,
        source,
        dedupe_key,
        scope_json,
        risk_level,
        roi_score,
        expected_gain,
        effort,
        acceptance_criteria_json,
    )) = row
    else {
        return Ok(None);
    };
    let Some(source) = parse_proposal_source(&source) else {
        return Ok(None);
    };
    let Some(risk_level) = parse_proposal_risk(&risk_level) else {
        return Ok(None);
    };
    let scope: EvolutionScope = serde_json::from_str(&scope_json).unwrap_or_default();
    let acceptance_criteria: Vec<String> =
        serde_json::from_str(&acceptance_criteria_json).unwrap_or_default();
    Ok(Some(EvolutionProposal {
        proposal_id,
        source,
        scope,
        risk_level,
        expected_gain,
        effort,
        roi_score,
        dedupe_key,
        acceptance_criteria,
    }))
}

fn load_backlog_proposal_by_dedupe_key(
    conn: &Connection,
    dedupe_key: &str,
) -> Result<Option<EvolutionProposal>> {
    let mut stmt = conn.prepare(
        "SELECT proposal_id, source, dedupe_key, scope_json, risk_level, roi_score, expected_gain, effort, acceptance_criteria
         FROM evolution_backlog
         WHERE dedupe_key = ?1 AND status != 'executed'
         ORDER BY updated_at DESC, proposal_id DESC
         LIMIT 1",
    )?;
    let row = stmt.query_row([dedupe_key], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, f32>(5)?,
            row.get::<_, f32>(6)?,
            row.get::<_, f32>(7)?,
            row.get::<_, String>(8)?,
        ))
    });
    let Ok((
        proposal_id,
        source,
        dedupe_key,
        scope_json,
        risk_level,
        roi_score,
        expected_gain,
        effort,
        acceptance_criteria_json,
    )) = row
    else {
        return Ok(None);
    };
    let Some(source) = parse_proposal_source(&source) else {
        return Ok(None);
    };
    let Some(risk_level) = parse_proposal_risk(&risk_level) else {
        return Ok(None);
    };
    let scope: EvolutionScope = serde_json::from_str(&scope_json).unwrap_or_default();
    let acceptance_criteria: Vec<String> =
        serde_json::from_str(&acceptance_criteria_json).unwrap_or_default();
    Ok(Some(EvolutionProposal {
        proposal_id,
        source,
        scope,
        risk_level,
        expected_gain,
        effort,
        roi_score,
        dedupe_key,
        acceptance_criteria,
    }))
}

fn parse_outcome_from_authorized_reason(reason: &str) -> Option<String> {
    let marker = "outcome=";
    let pos = reason.find(marker)?;
    let mut raw = reason[pos + marker.len()..].trim();
    if let Some(idx) = raw.find(',') {
        raw = &raw[..idx];
    }
    let normalized = raw.trim().to_ascii_lowercase();
    if normalized == "partial_success" || normalized == "failure" {
        Some(normalized)
    } else {
        None
    }
}

pub(crate) fn recover_forced_proposal_by_authorization_log(
    conn: &Connection,
    forced_proposal_id: &str,
) -> Result<Option<EvolutionProposal>> {
    let mut stmt = conn.prepare(
        "SELECT target_id, reason
         FROM evolution_log
         WHERE type = 'capability_evolution_authorized'
           AND reason LIKE '%' || ?1 || '%'
         ORDER BY ts DESC
         LIMIT 1",
    )?;
    let row = stmt.query_row([forced_proposal_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    });
    let Ok((tool_name, reason)) = row else {
        return Ok(None);
    };
    let Some(outcome) = parse_outcome_from_authorized_reason(&reason) else {
        return Ok(None);
    };
    let dedupe_key = format!(
        "user_capability:{}:{}",
        tool_name.trim().to_ascii_lowercase(),
        outcome
    );
    load_backlog_proposal_by_dedupe_key(conn, &dedupe_key)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AcceptanceThresholds {
    pub(crate) window_days: i64,
    pub(crate) min_success_rate: f64,
    pub(crate) max_correction_rate: f64,
    pub(crate) max_rollback_rate: f64,
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
    pub(crate) fn from_env() -> Self {
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

pub(crate) fn auto_link_acceptance_status(conn: &Connection, proposal_id: &str) -> Result<()> {
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

pub(crate) fn coordinate_proposals(
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

pub(crate) fn coordinate_proposals_with_config(
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
            if selected.risk_level == ProposalRiskLevel::Critical && config.deny_critical {
                let note = "Policy runtime disabled; critical risk denied by policy";
                set_backlog_status(
                    conn,
                    &selected.proposal_id,
                    "policy_denied",
                    "rejected",
                    note,
                )?;
                return Ok(CoordinatorDecision::Denied(selected));
            }
            let note =
                if config.auto_execute_low_risk && selected.risk_level == ProposalRiskLevel::Low {
                    "Policy runtime disabled; auto-execute low-risk"
                } else {
                    "Policy runtime disabled; direct execution"
                };
            set_backlog_status(conn, &selected.proposal_id, "executing", "pending", note)?;
            return Ok(CoordinatorDecision::Execute(selected));
        }

        let policy = evaluate_policy_runtime(conn, &selected, config)?;
        let note = summarize_policy_runtime(&policy);
        match policy.action {
            PolicyAction::Allow => {
                set_backlog_status(conn, &selected.proposal_id, "executing", "pending", &note)?;
                Ok(CoordinatorDecision::Execute(selected))
            }
            PolicyAction::Ask => {
                set_backlog_status(conn, &selected.proposal_id, "queued", "pending", &note)?;
                Ok(CoordinatorDecision::Queued(selected))
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

    // Only count real evolution runs for the daily cap. Scheduler "pings" that log
    // `evolution_run_outcome` (e.g. NoScope) must not consume the budget or they block forever.
    let today_evolutions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evolution_log
             WHERE date(ts) = date('now') AND type = 'evolution_run'",
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
        // Cooldown is time since the last *completed evolution run*, not since the last
        // `evolution_run_outcome` row (NoScope / SkippedBusy would otherwise reset cooldown
        // every scheduler tick and passive evolution never opens).
        let last_evo_hours: f64 = conn
            .query_row(
                "SELECT COALESCE(
                    (julianday('now') - julianday(MAX(ts))) * 24,
                    999.0
                ) FROM evolution_log WHERE type = 'evolution_run'",
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

#[cfg(test)]
mod describe_empty_proposals_tests {
    use super::*;
    use crate::feedback;
    use rusqlite::Connection;

    fn open_mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        feedback::ensure_evolution_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn describe_empty_when_mode_disabled() {
        let conn = open_mem();
        let mode = EvolutionMode::Disabled;
        assert_eq!(
            describe_empty_evolution_proposals(&conn, &mode, false).unwrap(),
            "NoScope: evolution disabled (SKILLLITE_EVOLUTION)"
        );
    }

    #[test]
    fn would_have_false_when_disabled() {
        let conn = open_mem();
        assert!(!would_have_evolution_proposals(&conn, EvolutionMode::Disabled, false).unwrap());
    }
}
