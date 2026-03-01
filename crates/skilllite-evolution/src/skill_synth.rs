//! Skill synthesis: auto-generate, refine, and retire skills (EVO-4).
//!
//! - **Generate**: detect repeated task patterns → LLM → SKILL.md + script → L4 scan + L5 sandbox
//! - **Refine**: failed skill → analyze error trace → LLM fix → retry (max 2 rounds)
//! - **Retire**: low success rate or unused skills → archive
//!
//! All evolved skills live in `chat/skills/_evolved/` with `.meta.json` metadata.
//! A10: Newly generated skills go to `_evolved/_pending/` until user confirms.

use std::path::Path;

use anyhow::Result;
use rusqlite::{params, Connection};

use skilllite_sandbox::security::scanner::ScriptScanner;

use crate::feedback;
use crate::gatekeeper_l1_path;
use crate::gatekeeper_l3_content;
use crate::log_evolution_event;
use crate::prompt_learner;
use crate::EvolutionLlm;
use crate::EvolutionMessage;

const SKILL_GENERATION_PROMPT: &str =
    include_str!("seed/evolution_prompts/skill_generation.seed.md");
const SKILL_REFINEMENT_PROMPT: &str =
    include_str!("seed/evolution_prompts/skill_refinement.seed.md");

const MAX_EVOLVED_SKILLS: usize = 20;
const MAX_REFINE_ROUNDS: usize = 2;
const RETIRE_UNUSED_DAYS: i64 = 30;
const RETIRE_LOW_SUCCESS_RATE: f64 = 0.30;

// ─── Skill metadata (persisted as .meta.json alongside each evolved skill) ──

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillMeta {
    pub name: String,
    pub source_session: String,
    pub created_at: String,
    pub success_count: u32,
    pub failure_count: u32,
    pub call_count: u32,
    pub last_used: Option<String>,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub generation_txn: String,
}

impl SkillMeta {
    fn success_rate(&self) -> f64 {
        if self.call_count == 0 {
            return 1.0;
        }
        self.success_count as f64 / self.call_count as f64
    }
}

// ─── Main entry: evolve skills ──────────────────────────────────────────────

/// Run skill evolution: generate new skills or refine existing ones.
/// Returns a list of (change_type, id) pairs for changelog.
pub async fn evolve_skills<L: EvolutionLlm>(
    chat_root: &Path,
    llm: &L,
    model: &str,
    txn_id: &str,
    generate: bool,
) -> Result<Vec<(String, String)>> {
    let mut changes = Vec::new();

    if generate {
        match generate_skill(chat_root, llm, model, txn_id).await {
            Ok(Some(name)) => {
                // A10: New skills enter pending queue until user confirms
                changes.push(("skill_pending".to_string(), name));
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("Skill generation failed: {}", e);
            }
        }
    } else {
        match refine_weakest_skill(chat_root, llm, model, txn_id).await {
            Ok(Some(name)) => {
                changes.push(("skill_refined".to_string(), name));
            }
            Ok(None) => {}
            Err(e) => {
                tracing::warn!("Skill refinement failed: {}", e);
            }
        }
    }

    // Retirement runs every cycle regardless
    let retired = retire_skills(chat_root, txn_id)?;
    changes.extend(retired);

    Ok(changes)
}

// ─── A10: Pending skill confirmation ────────────────────────────────────────

/// List skill names in the pending queue (awaiting user confirmation).
pub fn list_pending_skills(chat_root: &Path) -> Vec<String> {
    let pending_dir = chat_root.join("skills").join("_evolved").join("_pending");
    if !pending_dir.exists() {
        return Vec::new();
    }
    std::fs::read_dir(&pending_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir() && e.path().join("SKILL.md").exists())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect()
}

/// Move a pending skill to confirmed (_evolved/). Returns Ok(()) on success.
pub fn confirm_pending_skill(chat_root: &Path, skill_name: &str) -> Result<()> {
    let pending_dir = chat_root.join("skills").join("_evolved").join("_pending");
    let evolved_dir = chat_root.join("skills").join("_evolved");
    let src = pending_dir.join(skill_name);
    let dst = evolved_dir.join(skill_name);

    if !src.exists() {
        anyhow::bail!("待确认 Skill '{}' 不存在", skill_name);
    }
    if dst.exists() {
        anyhow::bail!("Skill '{}' 已存在，请先删除或重命名", skill_name);
    }

    std::fs::rename(&src, &dst)?;
    tracing::info!("Skill '{}' 已确认加入", skill_name);
    Ok(())
}

/// Reject (remove) a pending skill without adding it.
pub fn reject_pending_skill(chat_root: &Path, skill_name: &str) -> Result<()> {
    let pending_dir = chat_root.join("skills").join("_evolved").join("_pending");
    let src = pending_dir.join(skill_name);

    if !src.exists() {
        anyhow::bail!("待确认 Skill '{}' 不存在", skill_name);
    }

    std::fs::remove_dir_all(&src)?;
    tracing::info!("Skill '{}' 已拒绝", skill_name);
    Ok(())
}

// ─── Skill generation ─────────────────────────────────────────────────────

async fn generate_skill<L: EvolutionLlm>(
    chat_root: &Path,
    llm: &L,
    model: &str,
    txn_id: &str,
) -> Result<Option<String>> {
    let evolved_dir = chat_root.join("skills").join("_evolved");
    // A10: New skills go to _pending until user confirms
    let pending_dir = evolved_dir.join("_pending");

    // Check cap (count both confirmed and pending)
    let current_count = count_active_evolved_skills(&evolved_dir);
    if current_count >= MAX_EVOLVED_SKILLS {
        tracing::debug!(
            "Evolved skill cap reached ({}/{}), skipping generation",
            current_count, MAX_EVOLVED_SKILLS
        );
        return Ok(None);
    }

    // Query repeated patterns from decisions
    let (patterns, executions) = {
        let conn = feedback::open_evolution_db(chat_root)?;
        let patterns = query_repeated_patterns(&conn)?;
        let executions = if !patterns.is_empty() {
            query_pattern_executions(&conn, &patterns)?
        } else {
            String::new()
        };
        (patterns, executions)
    };

    if patterns.is_empty() {
        return Ok(None);
    }

    let existing_skills = list_existing_skill_names(chat_root);

    let prompt = SKILL_GENERATION_PROMPT
        .replace("{{repeated_patterns}}", &patterns)
        .replace("{{successful_executions}}", &executions)
        .replace("{{existing_skills}}", &existing_skills);

    let messages = vec![EvolutionMessage::user(&prompt)];
    let content = llm.complete(&messages, model, 0.3).await?.trim().to_string();

    let parsed = match parse_skill_generation_response(&content) {
        Ok(Some(s)) => s,
        Ok(None) => return Ok(None),
        Err(e) => {
            tracing::warn!("Failed to parse skill generation output: {}", e);
            return Ok(None);
        }
    };

    // L3 content check on both script and SKILL.md
    if let Err(e) = gatekeeper_l3_content(&parsed.script_content) {
        tracing::warn!("L3 rejected generated skill script: {}", e);
        return Ok(None);
    }
    if let Err(e) = gatekeeper_l3_content(&parsed.skill_md_content) {
        tracing::warn!("L3 rejected generated SKILL.md: {}", e);
        return Ok(None);
    }

    // A10: Write to _pending/ until user confirms
    let skill_dir = pending_dir.join(&parsed.name);
    if !gatekeeper_l1_path(chat_root, &skill_dir) {
        anyhow::bail!("L1 rejected skill directory: {}", skill_dir.display());
    }
    std::fs::create_dir_all(&skill_dir)?;

    let script_path = skill_dir.join(&parsed.entry_point);
    let skill_md_path = skill_dir.join("SKILL.md");

    // L4: security scan before writing
    let scan_result = run_l4_scan(&parsed.script_content, &script_path)?;
    if !scan_result {
        // Enter refinement loop
        tracing::info!(
            "L4 scan failed for generated skill '{}', entering refinement loop",
            parsed.name
        );
        let refined = refine_loop(
            chat_root,
            llm,
            model,
            &skill_dir,
            &parsed.name,
            &parsed.description,
            &parsed.entry_point,
            &parsed.script_content,
            "Security scan found critical/high issues",
            "security_scan",
        )
        .await?;

        match refined {
            Some(fixed_script) => {
                write_skill_files(
                    &skill_dir,
                    &skill_md_path,
                    &script_path,
                    &parsed.skill_md_content,
                    &fixed_script,
                    &parsed.name,
                    txn_id,
                )?;
            }
            None => {
                let _ = std::fs::remove_dir_all(&skill_dir);
                tracing::warn!("Skill '{}' abandoned after refinement loop", parsed.name);
                return Ok(None);
            }
        }
    } else {
        write_skill_files(
            &skill_dir,
            &skill_md_path,
            &script_path,
            &parsed.skill_md_content,
            &parsed.script_content,
            &parsed.name,
            txn_id,
        )?;
    }

    tracing::info!(
        "Generated evolved skill (pending confirmation): {}",
        parsed.name
    );
    Ok(Some(parsed.name))
}

fn write_skill_files(
    skill_dir: &Path,
    skill_md_path: &Path,
    script_path: &Path,
    skill_md: &str,
    script: &str,
    name: &str,
    txn_id: &str,
) -> Result<()> {
    std::fs::create_dir_all(skill_dir)?;
    std::fs::write(skill_md_path, skill_md)?;
    std::fs::write(script_path, script)?;

    // Make script executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(script_path, perms);
    }

    let meta = SkillMeta {
        name: name.to_string(),
        source_session: String::new(),
        created_at: chrono::Utc::now().to_rfc3339(),
        success_count: 0,
        failure_count: 0,
        call_count: 0,
        last_used: None,
        archived: false,
        generation_txn: txn_id.to_string(),
    };
    let meta_path = skill_dir.join(".meta.json");
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

    Ok(())
}

// ─── Refinement loop (Yunjue-inspired iterative refinement) ─────────────────

/// Retry fixing a skill script up to MAX_REFINE_ROUNDS times.
/// Each round sends the error trace to LLM for targeted fix.
async fn refine_loop<L: EvolutionLlm>(
    _chat_root: &Path,
    llm: &L,
    model: &str,
    skill_dir: &Path,
    skill_name: &str,
    skill_desc: &str,
    entry_point: &str,
    initial_script: &str,
    initial_error: &str,
    failure_type: &str,
) -> Result<Option<String>> {
    let mut current_script = initial_script.to_string();
    let mut current_error = initial_error.to_string();
    let script_path = skill_dir.join(entry_point);

    for round in 1..=MAX_REFINE_ROUNDS {
        tracing::info!(
            "Refinement round {}/{} for skill '{}'",
            round,
            MAX_REFINE_ROUNDS,
            skill_name
        );

        let prompt = SKILL_REFINEMENT_PROMPT
            .replace("{{skill_name}}", skill_name)
            .replace("{{skill_description}}", skill_desc)
            .replace("{{entry_point}}", entry_point)
            .replace("{{current_script}}", &current_script)
            .replace("{{error_trace}}", &current_error)
            .replace("{{failure_type}}", failure_type);

        let messages = vec![EvolutionMessage::user(&prompt)];
        let content = llm.complete(&messages, model, 0.3).await?.trim().to_string();

        let parsed = match parse_refinement_response(&content) {
            Ok(Some(r)) => r,
            Ok(None) => {
                tracing::info!("LLM skipped refinement for '{}': unfixable", skill_name);
                return Ok(None);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to parse refinement output (round {}): {}",
                    round,
                    e
                );
                return Ok(None);
            }
        };

        // L3 content check
        if let Err(e) = gatekeeper_l3_content(&parsed.fixed_script) {
            tracing::warn!("L3 rejected refined script (round {}): {}", round, e);
            return Ok(None);
        }

        // L4 scan
        let scan_ok = run_l4_scan(&parsed.fixed_script, &script_path)?;
        if scan_ok {
            tracing::info!(
                "Refinement succeeded for '{}' in round {}: {}",
                skill_name,
                round,
                parsed.fix_summary
            );
            return Ok(Some(parsed.fixed_script));
        }

        current_script = parsed.fixed_script;
        current_error = format!(
            "Previous fix attempt (round {}) still failed security scan. Summary: {}",
            round, parsed.fix_summary
        );
    }

    tracing::warn!(
        "Skill '{}' still failing after {} refinement rounds, abandoning",
        skill_name,
        MAX_REFINE_ROUNDS
    );
    Ok(None)
}

// ─── Refine weakest existing skill ──────────────────────────────────────────

async fn refine_weakest_skill<L: EvolutionLlm>(
    chat_root: &Path,
    llm: &L,
    model: &str,
    txn_id: &str,
) -> Result<Option<String>> {
    let evolved_dir = chat_root.join("skills").join("_evolved");
    if !evolved_dir.exists() {
        return Ok(None);
    }

    // Find the skill with lowest success rate (>= 3 calls, < 60% success)
    let mut weakest: Option<(String, SkillMeta, f64)> = None;

    for entry in std::fs::read_dir(&evolved_dir)?.flatten() {
        let skill_dir = entry.path();
        if !skill_dir.is_dir() {
            continue;
        }
        // A10: Skip _pending (meta directory)
        if entry.file_name().to_string_lossy().starts_with('_') {
            continue;
        }
        let meta_path = skill_dir.join(".meta.json");
        if !meta_path.exists() {
            continue;
        }
        let meta: SkillMeta = match std::fs::read_to_string(&meta_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(m) => m,
            None => continue,
        };
        if meta.archived || meta.call_count < 3 {
            continue;
        }
        let rate = meta.success_rate();
        if rate >= 0.60 {
            continue;
        }
        if weakest.as_ref().map_or(true, |(_, _, r)| rate < *r) {
            let name = entry.file_name().to_string_lossy().to_string();
            weakest = Some((name, meta, rate));
        }
    }

    let (skill_name, _meta, _rate) = match weakest {
        Some(w) => w,
        None => return Ok(None),
    };

    let skill_dir = evolved_dir.join(&skill_name);
    let skill_md_path = skill_dir.join("SKILL.md");
    let skill_md = std::fs::read_to_string(&skill_md_path).unwrap_or_default();

    // Find entry point
    let entry_point = detect_entry_point(&skill_dir);
    let script_path = skill_dir.join(&entry_point);
    let current_script = std::fs::read_to_string(&script_path).unwrap_or_default();

    if current_script.is_empty() {
        return Ok(None);
    }

    // Query recent failure traces
    let error_trace = {
        let conn = feedback::open_evolution_db(chat_root)?;
        query_skill_failures(&conn, &skill_name)?
    };

    if error_trace.is_empty() {
        return Ok(None);
    }

    let desc = extract_description_from_skill_md(&skill_md);

    let fixed = refine_loop(
        chat_root,
        llm,
        model,
        &skill_dir,
        &skill_name,
        &desc,
        &entry_point,
        &current_script,
        &error_trace,
        "execution_failure",
    )
    .await?;

    if let Some(fixed_script) = fixed {
        std::fs::write(&script_path, &fixed_script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(
                &script_path,
                std::fs::Permissions::from_mode(0o755),
            );
        }

        // Log evolution event
        if let Ok(conn) = feedback::open_evolution_db(chat_root) {
            let _ = log_evolution_event(
                &conn,
                chat_root,
                "skill_refined",
                &skill_name,
                "Refined after low success rate",
                txn_id,
            );
        }

        tracing::info!("Refined evolved skill: {}", skill_name);
        return Ok(Some(skill_name));
    }

    Ok(None)
}

// ─── Skill retirement ───────────────────────────────────────────────────────

fn retire_skills(chat_root: &Path, txn_id: &str) -> Result<Vec<(String, String)>> {
    let evolved_dir = chat_root.join("skills").join("_evolved");
    if !evolved_dir.exists() {
        return Ok(Vec::new());
    }

    let mut retired = Vec::new();
    let now = chrono::Utc::now();

    for entry in std::fs::read_dir(&evolved_dir)?.flatten() {
        let skill_dir = entry.path();
        if !skill_dir.is_dir() {
            continue;
        }
        // A10: Skip _pending (meta directory)
        if entry.file_name().to_string_lossy().starts_with('_') {
            continue;
        }
        let meta_path = skill_dir.join(".meta.json");
        if !meta_path.exists() {
            continue;
        }
        let mut meta: SkillMeta = match std::fs::read_to_string(&meta_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(m) => m,
            None => continue,
        };
        if meta.archived {
            continue;
        }

        let should_retire = if meta.call_count >= 3
            && meta.success_rate() < RETIRE_LOW_SUCCESS_RATE
        {
            Some(format!(
                "success rate {:.0}% < {:.0}% threshold",
                meta.success_rate() * 100.0,
                RETIRE_LOW_SUCCESS_RATE * 100.0,
            ))
        } else if let Some(ref last) = meta.last_used {
            if let Ok(last_dt) = chrono::DateTime::parse_from_rfc3339(last) {
                let days = (now - last_dt.with_timezone(&chrono::Utc)).num_days();
                if days >= RETIRE_UNUSED_DAYS {
                    Some(format!(
                        "unused for {} days (threshold: {})",
                        days, RETIRE_UNUSED_DAYS
                    ))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            // Never used — check creation date
            if let Ok(created) = chrono::DateTime::parse_from_rfc3339(&meta.created_at) {
                let days = (now - created.with_timezone(&chrono::Utc)).num_days();
                if days >= RETIRE_UNUSED_DAYS {
                    Some(format!("never used, {} days since creation", days))
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(reason) = should_retire {
            meta.archived = true;
            let _ = std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?);

            let name = entry.file_name().to_string_lossy().to_string();
            tracing::info!("Retired skill '{}': {}", name, reason);

            if let Ok(conn) = feedback::open_evolution_db(chat_root) {
                let _ = log_evolution_event(&conn, chat_root, "skill_retired", &name, &reason, txn_id);
            }

            retired.push(("skill_retired".to_string(), name));
        }
    }

    Ok(retired)
}

// ─── L4 security scan ───────────────────────────────────────────────────────

fn run_l4_scan(script_content: &str, script_path: &Path) -> Result<bool> {
    let scanner = ScriptScanner::new();
    let result = scanner.scan_content(script_content, script_path)?;
    if !result.is_safe {
        tracing::warn!(
            "L4 security scan found issues in {}",
            script_path.display()
        );
    }
    Ok(result.is_safe)
}

// ─── Skill usage tracking ───────────────────────────────────────────────────

/// Update .meta.json after a skill execution (called from agent_loop).
pub fn track_skill_usage(evolved_dir: &Path, skill_name: &str, success: bool) {
    let meta_path = evolved_dir.join(skill_name).join(".meta.json");
    if !meta_path.exists() {
        return;
    }
    let mut meta: SkillMeta = match std::fs::read_to_string(&meta_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
    {
        Some(m) => m,
        None => return,
    };

    meta.call_count += 1;
    if success {
        meta.success_count += 1;
    } else {
        meta.failure_count += 1;
    }
    meta.last_used = Some(chrono::Utc::now().to_rfc3339());

    let _ =
        std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap_or_default());
}

// ─── Query helpers ──────────────────────────────────────────────────────────

fn query_repeated_patterns(conn: &Connection) -> Result<String> {
    let mut stmt = conn.prepare(
        "SELECT task_description, COUNT(*) as cnt,
                SUM(CASE WHEN task_completed = 1 THEN 1 ELSE 0 END) as successes
         FROM decisions
         WHERE evolved = 0 AND task_description IS NOT NULL
         GROUP BY task_description
         HAVING cnt >= 3 AND CAST(successes AS REAL) / cnt >= 0.8
         ORDER BY cnt DESC LIMIT 5",
    )?;

    let rows: Vec<String> = stmt
        .query_map([], |row| {
            let desc: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            let succ: i64 = row.get(2)?;
            Ok(format!(
                "- 模式: {} | 出现: {}次 | 成功: {}次 ({:.0}%)",
                desc,
                cnt,
                succ,
                succ as f64 / cnt as f64 * 100.0
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows.join("\n"))
}

fn query_pattern_executions(conn: &Connection, _patterns: &str) -> Result<String> {
    let mut stmt = conn.prepare(
        "SELECT task_description, tools_detail, elapsed_ms
         FROM decisions
         WHERE evolved = 0 AND task_completed = 1 AND task_description IS NOT NULL
         ORDER BY ts DESC LIMIT 10",
    )?;

    let rows: Vec<String> = stmt
        .query_map([], |row| {
            let desc: String = row.get(0)?;
            let tools: Option<String> = row.get(1)?;
            let elapsed: i64 = row.get(2)?;
            Ok(format!(
                "- 任务: {} | 工具: {} | 耗时: {}ms",
                desc,
                tools.unwrap_or_else(|| "N/A".to_string()),
                elapsed
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows.join("\n"))
}

fn query_skill_failures(conn: &Connection, skill_name: &str) -> Result<String> {
    let tool_pattern = format!("%{}%", skill_name);
    let mut stmt = conn.prepare(
        "SELECT task_description, tools_detail, feedback
         FROM decisions
         WHERE failed_tools > 0 AND tools_detail LIKE ?1
         ORDER BY ts DESC LIMIT 5",
    )?;

    let rows: Vec<String> = stmt
        .query_map(params![tool_pattern], |row| {
            let desc: Option<String> = row.get(0)?;
            let tools: Option<String> = row.get(1)?;
            let fb: Option<String> = row.get(2)?;
            Ok(format!(
                "- 任务: {} | 工具详情: {} | 反馈: {}",
                desc.unwrap_or_default(),
                tools.unwrap_or_default(),
                fb.unwrap_or_default(),
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows.join("\n"))
}

// ─── Listing helpers ────────────────────────────────────────────────────────

fn list_existing_skill_names(chat_root: &Path) -> String {
    let evolved_dir = chat_root.join("skills").join("_evolved");
    if !evolved_dir.exists() {
        return "(无已有 Skill)".to_string();
    }

    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&evolved_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('_') {
            if name == "_pending" && path.is_dir() {
                for e in std::fs::read_dir(&path)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                {
                    if e.path().is_dir() && e.path().join("SKILL.md").exists() {
                        names.push(format!("- {}", e.file_name().to_string_lossy()));
                    }
                }
            }
            continue;
        }
        if path.is_dir() && path.join("SKILL.md").exists() {
            names.push(format!("- {}", name));
        }
    }

    if names.is_empty() {
        "(无已有 Skill)".to_string()
    } else {
        names.join("\n")
    }
}

fn count_active_evolved_skills(evolved_dir: &Path) -> usize {
    if !evolved_dir.exists() {
        return 0;
    }
    let mut count = 0;
    for entry in std::fs::read_dir(evolved_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        // Skip meta dirs like _pending when counting top-level
        if name.starts_with('_') {
            // Count skills inside _pending (A10)
            if name == "_pending" && path.is_dir() {
                count += std::fs::read_dir(&path)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().join(".meta.json").exists())
                    .count();
            }
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        let meta_path = path.join(".meta.json");
        if !meta_path.exists() {
            continue;
        }
        if std::fs::read_to_string(&meta_path)
            .ok()
            .and_then(|s| serde_json::from_str::<SkillMeta>(&s).ok())
            .map(|m| !m.archived)
            .unwrap_or(true)
        {
            count += 1;
        }
    }
    count
}

fn detect_entry_point(skill_dir: &Path) -> String {
    for candidate in &["main.py", "scripts/main.py"] {
        if skill_dir.join(candidate).exists() {
            return candidate.to_string();
        }
    }
    "main.py".to_string()
}

fn extract_description_from_skill_md(content: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("---") {
            return trimmed.to_string();
        }
    }
    String::new()
}

// ─── Response parsing ──────────────────────────────────────────────────────

struct GeneratedSkill {
    name: String,
    description: String,
    entry_point: String,
    script_content: String,
    skill_md_content: String,
}

struct RefinedSkill {
    fixed_script: String,
    fix_summary: String,
}

fn parse_skill_generation_response(content: &str) -> Result<Option<GeneratedSkill>> {
    let json_str = prompt_learner::extract_json_block(content);

    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| anyhow::anyhow!("Failed to parse skill generation JSON: {}", e))?;

    if let Some(skip) = parsed.get("skip_reason").and_then(|v| v.as_str()) {
        if !skip.is_empty() && skip != "null" {
            tracing::debug!("Skill generation skipped: {}", skip);
            return Ok(None);
        }
    }

    let skill = parsed
        .get("skill")
        .ok_or_else(|| anyhow::anyhow!("No 'skill' field in response"))?;

    let name = skill.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    if name.is_empty() || name.len() > 50 {
        return Ok(None);
    }

    let description = skill.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let entry_point = skill
        .get("entry_point")
        .and_then(|v| v.as_str())
        .unwrap_or("main.py")
        .to_string();
    let script_content = skill
        .get("script_content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let skill_md_content = skill
        .get("skill_md_content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if script_content.is_empty() || skill_md_content.is_empty() {
        return Ok(None);
    }

    // Line count check (≤150 lines)
    if script_content.lines().count() > 150 {
        tracing::warn!("Generated script exceeds 150 lines, rejecting");
        return Ok(None);
    }

    Ok(Some(GeneratedSkill {
        name,
        description,
        entry_point,
        script_content,
        skill_md_content,
    }))
}

fn parse_refinement_response(content: &str) -> Result<Option<RefinedSkill>> {
    let json_str = prompt_learner::extract_json_block(content);

    let parsed: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| anyhow::anyhow!("Failed to parse refinement JSON: {}", e))?;

    if let Some(skip) = parsed.get("skip_reason").and_then(|v| v.as_str()) {
        if !skip.is_empty() && skip != "null" {
            tracing::debug!("Refinement skipped: {}", skip);
            return Ok(None);
        }
    }

    let fixed_script = parsed
        .get("fixed_script")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let fix_summary = parsed
        .get("fix_summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if fixed_script.is_empty() {
        return Ok(None);
    }

    if fixed_script.lines().count() > 150 {
        tracing::warn!("Refined script exceeds 150 lines, rejecting");
        return Ok(None);
    }

    Ok(Some(RefinedSkill {
        fixed_script,
        fix_summary,
    }))
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_skill_generation() {
        let json = serde_json::json!({
            "skill": {
                "name": "daily-report",
                "description": "Generate daily work summary",
                "entry_point": "main.py",
                "input_schema": {"type": "object", "properties": {}},
                "script_content": "#!/usr/bin/env python3\nimport sys\nprint('hello')",
                "skill_md_content": "# Skill: daily-report\n\n## Description\nGenerate daily summary"
            },
            "skip_reason": null
        })
        .to_string();

        let result = parse_skill_generation_response(&json).unwrap();
        assert!(result.is_some());
        let skill = result.unwrap();
        assert_eq!(skill.name, "daily-report");
        assert!(!skill.script_content.is_empty());
    }

    #[test]
    fn test_parse_skill_generation_skipped() {
        let json =
            serde_json::json!({"skill": null, "skip_reason": "no repeated pattern"}).to_string();
        let result = parse_skill_generation_response(&json).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_skill_generation_too_long() {
        let long_script = (0..200).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let json = serde_json::json!({
            "skill": {
                "name": "test",
                "description": "t",
                "entry_point": "main.py",
                "script_content": long_script,
                "skill_md_content": "# test"
            },
            "skip_reason": null
        })
        .to_string();
        let result = parse_skill_generation_response(&json).unwrap();
        assert!(result.is_none(), "should reject scripts > 150 lines");
    }

    #[test]
    fn test_parse_refinement_response() {
        let json = serde_json::json!({
            "fixed_script": "#!/usr/bin/env python3\nimport sys\nprint('fixed')",
            "fix_summary": "Removed unsafe eval call",
            "skip_reason": null
        })
        .to_string();

        let result = parse_refinement_response(&json).unwrap();
        assert!(result.is_some());
        let refined = result.unwrap();
        assert!(refined.fixed_script.contains("fixed"));
        assert_eq!(refined.fix_summary, "Removed unsafe eval call");
    }

    #[test]
    fn test_parse_refinement_unfixable() {
        let json = serde_json::json!({"fixed_script": "", "fix_summary": "", "skip_reason": "fundamental design flaw"}).to_string();
        let result = parse_refinement_response(&json).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_skill_meta_success_rate() {
        let meta = SkillMeta {
            name: "test".into(),
            source_session: String::new(),
            created_at: String::new(),
            success_count: 7,
            failure_count: 3,
            call_count: 10,
            last_used: None,
            archived: false,
            generation_txn: String::new(),
        };
        assert!((meta.success_rate() - 0.7).abs() < f64::EPSILON);

        let empty = SkillMeta {
            call_count: 0,
            ..meta.clone()
        };
        assert!((empty.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_count_active_evolved_skills() {
        let tmp = tempfile::TempDir::new().unwrap();
        let evolved_dir = tmp.path().join("_evolved");
        std::fs::create_dir_all(&evolved_dir).unwrap();

        // Active skill
        let s1 = evolved_dir.join("skill-a");
        std::fs::create_dir_all(&s1).unwrap();
        let meta1 = SkillMeta {
            name: "skill-a".into(),
            source_session: String::new(),
            created_at: String::new(),
            success_count: 5,
            failure_count: 0,
            call_count: 5,
            last_used: None,
            archived: false,
            generation_txn: String::new(),
        };
        std::fs::write(s1.join(".meta.json"), serde_json::to_string(&meta1).unwrap()).unwrap();

        // Archived skill
        let s2 = evolved_dir.join("skill-b");
        std::fs::create_dir_all(&s2).unwrap();
        let meta2 = SkillMeta {
            archived: true,
            name: "skill-b".into(),
            ..meta1.clone()
        };
        std::fs::write(s2.join(".meta.json"), serde_json::to_string(&meta2).unwrap()).unwrap();

        assert_eq!(count_active_evolved_skills(&evolved_dir), 1);
    }

    #[test]
    fn test_retire_skills() {
        let tmp = tempfile::TempDir::new().unwrap();
        let chat_root = tmp.path();
        let evolved_dir = chat_root.join("skills").join("_evolved");
        std::fs::create_dir_all(&evolved_dir).unwrap();

        // Low success rate skill (should be retired)
        let s1 = evolved_dir.join("bad-skill");
        std::fs::create_dir_all(&s1).unwrap();
        let meta = SkillMeta {
            name: "bad-skill".into(),
            source_session: String::new(),
            created_at: chrono::Utc::now().to_rfc3339(),
            success_count: 1,
            failure_count: 9,
            call_count: 10,
            last_used: Some(chrono::Utc::now().to_rfc3339()),
            archived: false,
            generation_txn: String::new(),
        };
        std::fs::write(
            s1.join(".meta.json"),
            serde_json::to_string_pretty(&meta).unwrap(),
        )
        .unwrap();

        // Also create the memory dir and DB for logging
        let mem_dir = chat_root.join("memory");
        std::fs::create_dir_all(&mem_dir).unwrap();

        let retired = retire_skills(chat_root, "test_txn").unwrap();
        assert_eq!(retired.len(), 1);
        assert_eq!(retired[0].1, "bad-skill");

        // Verify meta.archived is now true
        let updated: SkillMeta =
            serde_json::from_str(&std::fs::read_to_string(s1.join(".meta.json")).unwrap()).unwrap();
        assert!(updated.archived);
    }

    #[test]
    fn test_l4_scan_safe() {
        let tmp = tempfile::TempDir::new().unwrap();
        let script = "#!/usr/bin/env python3\nimport json\nprint(json.dumps({'ok': True}))";
        let path = tmp.path().join("test.py");
        let result = run_l4_scan(script, &path).unwrap();
        assert!(result, "simple safe script should pass L4");
    }

    #[test]
    fn test_extract_description() {
        let md = "# Skill: test\n\nA useful skill for testing.\n\n## Input\n...";
        let desc = extract_description_from_skill_md(md);
        assert_eq!(desc, "A useful skill for testing.");
    }
}
