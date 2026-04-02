//! 技能、进化、引导、运行时、Ollama、日程。

use serde::Serialize;
use skilllite_core::skill::manifest;
use std::fs;
use std::path::PathBuf;
use tauri::Emitter;

use super::paths::{find_project_root, load_dotenv_for_child};

// ─── List skills & repair-skills (evolution) ───────────────────────────────────

/// Whether the skill dir has any script file (scripts/ or root with common script extensions).
fn skill_has_scripts(path: &std::path::Path) -> bool {
    let scripts_dir = path.join("scripts");
    if scripts_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            if entries
                .filter(|e| e.as_ref().ok().map(|e| e.path().is_file()).unwrap_or(false))
                .count()
                > 0
            {
                return true;
            }
        }
    }
    const EXTS: &[&str] = &["py", "js", "ts", "sh", "bash"];
    if let Ok(entries) = std::fs::read_dir(path) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_file() {
                if let Some(ext) = p.extension() {
                    if EXTS.contains(&ext.to_string_lossy().to_lowercase().as_str()) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Collect skill (dir_path, name) from a root dir, same shape as evolution validate (including _evolved/_pending).
fn collect_skill_dirs(root: &std::path::Path) -> Vec<(PathBuf, String)> {
    if !root.exists() || !root.is_dir() {
        return Vec::new();
    }
    let mut dirs = Vec::new();
    for e in std::fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
    {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with('_') {
            if name == "_evolved" || name == "_pending" {
                for e2 in std::fs::read_dir(&path)
                    .ok()
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                {
                    let p2 = e2.path();
                    let sub = e2.file_name().to_string_lossy().into_owned();
                    if !p2.is_dir() {
                        continue;
                    }
                    if p2.join("SKILL.md").exists() && skill_has_scripts(&p2) {
                        dirs.push((p2, sub));
                    } else if sub == "_pending" {
                        for e3 in std::fs::read_dir(&p2)
                            .ok()
                            .into_iter()
                            .flatten()
                            .filter_map(|e| e.ok())
                        {
                            let p3 = e3.path();
                            if p3.is_dir() && p3.join("SKILL.md").exists() && skill_has_scripts(&p3)
                            {
                                dirs.push((p3, e3.file_name().to_string_lossy().into_owned()));
                            }
                        }
                    }
                }
            } else if path.join("SKILL.md").exists() && skill_has_scripts(&path) {
                dirs.push((path, name));
            }
            continue;
        }
        if path.join("SKILL.md").exists() && skill_has_scripts(&path) {
            dirs.push((path, name));
        }
    }
    dirs
}

/// List skill names in workspace (for repair UI). Uses same logic as evolution: .skills and skills, incl. _evolved/_pending.
pub fn list_skill_names(workspace: &str) -> Vec<String> {
    let root = find_project_root(workspace);
    let mut names = std::collections::HashSet::new();
    for skills_sub in [".skills", "skills"] {
        let dir = root.join(skills_sub);
        for (_, name) in collect_skill_dirs(&dir) {
            names.insert(name);
        }
    }
    let mut v: Vec<String> = names.into_iter().collect();
    v.sort();
    v
}

/// Resolve skill directory path by name (searches .skills and skills, incl. _evolved/_pending). Returns None if not found.
fn find_skill_dir(workspace: &str, skill_name: &str) -> Option<std::path::PathBuf> {
    let root = find_project_root(workspace);
    for skills_sub in [".skills", "skills"] {
        let dir = root.join(skills_sub);
        for (path, name) in collect_skill_dirs(&dir) {
            if name == skill_name {
                return Some(path);
            }
        }
    }
    None
}

/// Open the given skill's directory in the system file manager.
pub fn open_skill_directory(workspace: &str, skill_name: &str) -> Result<(), String> {
    let path = find_skill_dir(workspace, skill_name)
        .ok_or_else(|| format!("未找到技能目录: {}", skill_name))?;
    if !path.exists() || !path.is_dir() {
        return Err(format!("技能目录不存在: {}", path.display()));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path.to_string_lossy().to_string())
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Remove installed skills from `.skills` or `skills` under the workspace (same discovery as list/open).
/// Updates `.skilllite-manifest.json` when present. `skill_names` must be non-empty.
pub fn remove_skills(workspace: &str, skill_names: &[String]) -> Result<String, String> {
    if skill_names.is_empty() {
        return Err("请至少勾选一个要删除的技能".to_string());
    }
    let mut lines: Vec<String> = Vec::new();
    let mut deleted = 0usize;
    for name in skill_names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let Some(skill_path) = find_skill_dir(workspace, name) else {
            lines.push(format!("未找到技能，已跳过: {}", name));
            continue;
        };
        let skills_parent = skill_path
            .parent()
            .ok_or_else(|| format!("无效技能路径: {}", skill_path.display()))?;
        manifest::remove_skill_entry(skills_parent, &skill_path).map_err(|e| e.to_string())?;
        fs::remove_dir_all(&skill_path).map_err(|e| {
            format!(
                "删除目录失败 {}: {}",
                skill_path.display(),
                e
            )
        })?;
        deleted += 1;
        lines.push(format!("已删除: {}", name));
    }
    if deleted == 0 {
        return Err(if lines.is_empty() {
            "没有可删除的技能".to_string()
        } else {
            lines.join("\n")
        });
    }
    Ok(lines.join("\n"))
}

/// Run `skilllite evolution repair-skills [skill_names...]`. If skill_names is empty, repairs all failed; otherwise only those.
pub fn repair_skills(
    workspace: &str,
    skill_names: &[String],
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let root = find_project_root(workspace);

    let mut cmd = std::process::Command::new(skilllite_path);
    cmd.arg("evolution").arg("repair-skills");
    for name in skill_names {
        cmd.arg(name);
    }
    cmd.arg("--from-source");
    cmd.current_dir(&root)
        .env("SKILLLITE_WORKSPACE", root.to_string_lossy().as_ref())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    for (k, v) in load_dotenv_for_child(workspace) {
        cmd.env(k, v);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("执行 repair-skills 失败: {}", e))?;
    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    let combined = if err.is_empty() {
        out.trim().to_string()
    } else {
        format!("{}\n{}", out.trim(), err.trim())
    };
    if !output.status.success() {
        return Err(combined);
    }
    Ok(combined)
}

/// Run `skilllite add <source>` in the workspace. Installs to workspace .skills (creates if needed).
/// Source: owner/repo, owner/repo@skill-name, https://github.com/..., or local path.
pub fn add_skill(
    workspace: &str,
    source: &str,
    force: bool,
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let root = find_project_root(workspace);
    let source = source.trim();
    if source.is_empty() {
        return Err("请填写来源，例如：owner/repo 或 owner/repo@skill-name".to_string());
    }

    let mut cmd = std::process::Command::new(skilllite_path);
    cmd.arg("add")
        .arg(source)
        .arg("--skills-dir")
        .arg(".skills");
    if force {
        cmd.arg("--force");
    }
    cmd.current_dir(&root)
        .env("SKILLLITE_WORKSPACE", root.to_string_lossy().as_ref())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    for (k, v) in load_dotenv_for_child(workspace) {
        cmd.env(k, v);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("执行 skilllite add 失败: {}", e))?;
    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    let combined = if err.is_empty() {
        out.trim().to_string()
    } else {
        format!("{}\n{}", out.trim(), err.trim())
    };
    if !output.status.success() {
        return Err(combined);
    }
    Ok(summarise_add_output(&combined))
}

/// 从 skilllite add 的完整输出中提取简短摘要，避免在桌面端刷屏。
fn summarise_add_output(output: &str) -> String {
    if output.is_empty() {
        return "已添加".to_string();
    }
    // 匹配 "🎉 Successfully added 14 skill(s) from obra/superpowers" 或 "Successfully added 1 skill(s)"
    let line = output
        .lines()
        .find(|line| line.contains("Successfully added") && line.contains("skill(s)"));
    if let Some(line) = line {
        let line = line.trim().trim_start_matches("🎉 ").trim();
        if let Some(after) = line.strip_prefix("Successfully added ") {
            let num_str = after.split_whitespace().next().unwrap_or("");
            if let Ok(n) = num_str.parse::<u32>() {
                let from = after.split(" from ").nth(1).map(str::trim);
                return if let Some(src) = from {
                    format!("已添加 {} 个技能（来自 {}）", n, src)
                } else {
                    format!("已添加 {} 个技能", n)
                };
            }
        }
    }
    "已添加".to_string()
}

// ─── Evolution status & pending skill review (desktop) ───────────────────────

fn workspace_env_lookup(workspace: &str, key: &str) -> Option<String> {
    load_dotenv_for_child(workspace)
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.clone())
        .or_else(|| std::env::var(key).ok())
}

fn evolution_mode_from_workspace(workspace: &str) -> skilllite_evolution::EvolutionMode {
    use skilllite_core::config::env_keys::evolution as evo_env;
    match workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION).as_deref() {
        None | Some("1") | Some("true") | Some("") => skilllite_evolution::EvolutionMode::All,
        Some("0") | Some("false") => skilllite_evolution::EvolutionMode::Disabled,
        Some("prompts") => skilllite_evolution::EvolutionMode::PromptsOnly,
        Some("memory") => skilllite_evolution::EvolutionMode::MemoryOnly,
        Some("skills") => skilllite_evolution::EvolutionMode::SkillsOnly,
        _ => skilllite_evolution::EvolutionMode::All,
    }
}

/// Desktop **Life Pulse** only: whether to spawn `skilllite evolution run`.
///
/// Matches the evolution panel copy (A9): periodic interval **or** unprocessed decisions ≥ threshold.
/// Does **not** use passive `should_evolve` heuristics — the subprocess runs and may return `NoScope`.
///
/// `last_periodic_spawn_unix`: last time the **periodic** arm fired; updated when the periodic
/// condition is met. Initialized lazily on first check so the first periodic window starts then.
pub fn evolution_growth_due(
    workspace: &str,
    last_periodic_spawn_unix: &std::sync::Mutex<Option<i64>>,
) -> bool {
    let mode = evolution_mode_from_workspace(workspace);
    if mode.is_disabled() {
        return false;
    }
    use skilllite_core::config::env_keys::evolution as evo_env;
    let interval_secs: u64 = workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION_INTERVAL_SECS)
        .and_then(|v| v.parse().ok())
        .unwrap_or(1800);
    let threshold: i64 =
        workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION_DECISION_THRESHOLD)
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

    let chat_root = skilllite_core::paths::chat_root();
    let count: i64 = skilllite_evolution::feedback::open_evolution_db(&chat_root)
        .ok()
        .and_then(|conn| skilllite_evolution::feedback::count_unprocessed_decisions(&conn).ok())
        .unwrap_or(0);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let need_count = count >= threshold;

    let mut g = last_periodic_spawn_unix
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let last_ts = match *g {
        None => {
            *g = Some(now);
            now
        }
        Some(t) => t,
    };
    let need_periodic = now.saturating_sub(last_ts) >= interval_secs as i64;

    if !need_count && !need_periodic {
        return false;
    }
    if need_periodic {
        *g = Some(now);
    }
    true
}

fn evolution_mode_labels(mode: &skilllite_evolution::EvolutionMode) -> (&'static str, &'static str) {
    match mode {
        skilllite_evolution::EvolutionMode::All => ("all", "全部启用"),
        skilllite_evolution::EvolutionMode::PromptsOnly => ("prompts", "仅 Prompts"),
        skilllite_evolution::EvolutionMode::MemoryOnly => ("memory", "仅 Memory"),
        skilllite_evolution::EvolutionMode::SkillsOnly => ("skills", "仅 Skills"),
        skilllite_evolution::EvolutionMode::Disabled => ("disabled", "已禁用"),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionLogEntryDto {
    pub ts: String,
    #[serde(rename = "event_type")]
    pub event_type: String,
    pub target_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionStatusPayload {
    pub mode_key: String,
    pub mode_label: String,
    pub interval_secs: u64,
    pub decision_threshold: i64,
    pub unprocessed_decisions: i64,
    pub last_run_ts: Option<String>,
    pub judgement_label: Option<String>,
    pub judgement_reason: Option<String>,
    pub recent_events: Vec<EvolutionLogEntryDto>,
    pub pending_skill_count: usize,
    pub db_error: Option<String>,
}

/// Evolution feedback DB + schedule hints for the assistant UI.
pub fn load_evolution_status(workspace: &str) -> EvolutionStatusPayload {
    use skilllite_core::config::env_keys::evolution as evo_env;
    let mode = evolution_mode_from_workspace(workspace);
    let (mode_key, mode_label) = evolution_mode_labels(&mode);

    let interval_secs: u64 = workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION_INTERVAL_SECS)
        .and_then(|v| v.parse().ok())
        .unwrap_or(1800);
    let decision_threshold: i64 =
        workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION_DECISION_THRESHOLD)
            .and_then(|v| v.parse().ok())
            .unwrap_or(10);

    let chat_root = skilllite_core::paths::chat_root();
    let mut pending_skill_count = 0;
    let skills_root = find_project_root(workspace).join(".skills");
    if skills_root.is_dir() {
        pending_skill_count =
            skilllite_evolution::skill_synth::list_pending_skills_with_review(&skills_root).len();
    }

    let mut db_error = None;
    let mut unprocessed_decisions = 0i64;
    let mut recent_events = Vec::new();
    let mut last_run_ts = None;
    let mut judgement_label = None;
    let mut judgement_reason = None;

    match skilllite_evolution::feedback::open_evolution_db(&chat_root) {
        Ok(conn) => {
            if let Ok(c) = skilllite_evolution::feedback::count_unprocessed_decisions(&conn) {
                unprocessed_decisions = c;
            }
            if let Ok(Some(summary)) = skilllite_evolution::feedback::build_latest_judgement(&conn) {
                judgement_label = Some(summary.judgement.label_zh().to_string());
                judgement_reason = Some(summary.reason);
            }
            if let Ok(mut stmt) = conn.prepare(
                "SELECT ts FROM evolution_log WHERE type = 'evolution_run' ORDER BY ts DESC LIMIT 1",
            ) {
                if let Ok(mut rows) = stmt.query([]) {
                    if let Ok(Some(row)) = rows.next() {
                        last_run_ts = row.get(0).ok();
                    }
                }
            }
            if let Ok(mut stmt) = conn.prepare(
                "SELECT ts, type, target_id, reason FROM evolution_log ORDER BY ts DESC LIMIT 25",
            ) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    Ok(EvolutionLogEntryDto {
                        ts: row.get(0)?,
                        event_type: row.get(1)?,
                        target_id: row.get::<_, Option<String>>(2)?,
                        reason: row.get::<_, Option<String>>(3)?,
                    })
                }) {
                    recent_events.extend(rows.flatten());
                }
            }
        }
        Err(e) => {
            db_error = Some(format!("无法打开进化数据库: {}", e));
        }
    }

    EvolutionStatusPayload {
        mode_key: mode_key.to_string(),
        mode_label: mode_label.to_string(),
        interval_secs,
        decision_threshold,
        unprocessed_decisions,
        last_run_ts,
        judgement_label,
        judgement_reason,
        recent_events,
        pending_skill_count,
        db_error,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PendingSkillDto {
    pub name: String,
    pub needs_review: bool,
    pub preview: String,
}

fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}

pub fn list_evolution_pending_skills(workspace: &str) -> Vec<PendingSkillDto> {
    let skills_root = find_project_root(workspace).join(".skills");
    if !skills_root.is_dir() {
        return Vec::new();
    }
    skilllite_evolution::skill_synth::list_pending_skills_with_review(&skills_root)
        .into_iter()
        .map(|(name, needs_review)| {
            let path = skills_root
                .join("_evolved")
                .join("_pending")
                .join(&name)
                .join("SKILL.md");
            let preview = std::fs::read_to_string(&path)
                .map(|s| truncate_utf8(&s, 4000))
                .unwrap_or_default();
            PendingSkillDto {
                name,
                needs_review,
                preview,
            }
        })
        .collect()
}

pub fn read_evolution_pending_skill_md(workspace: &str, skill_name: &str) -> Result<String, String> {
    let skills_root = find_project_root(workspace).join(".skills");
    let path = skills_root
        .join("_evolved")
        .join("_pending")
        .join(skill_name)
        .join("SKILL.md");
    if !path.is_file() {
        return Err(format!("未找到待审核技能: {}", skill_name));
    }
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

pub fn evolution_confirm_pending_skill(workspace: &str, skill_name: &str) -> Result<(), String> {
    let skills_root = find_project_root(workspace).join(".skills");
    skilllite_evolution::skill_synth::confirm_pending_skill(&skills_root, skill_name)
        .map_err(|e| e.to_string())?;
    let chat_root = skilllite_core::paths::chat_root();
    if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&chat_root) {
        let _ = skilllite_evolution::log_evolution_event(
            &conn,
            &chat_root,
            "skill_confirmed",
            skill_name,
            "user confirmed (assistant)",
            "",
        );
    }
    Ok(())
}

pub fn evolution_reject_pending_skill(workspace: &str, skill_name: &str) -> Result<(), String> {
    let skills_root = find_project_root(workspace).join(".skills");
    skilllite_evolution::skill_synth::reject_pending_skill(&skills_root, skill_name)
        .map_err(|e| e.to_string())
}

pub fn authorize_capability_evolution(
    workspace: &str,
    tool_name: &str,
    outcome: &str,
    summary: &str,
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let chat_root = skilllite_core::paths::chat_root();
    let conn =
        skilllite_evolution::feedback::open_evolution_db(&chat_root).map_err(|e| e.to_string())?;
    let proposal_id =
        skilllite_evolution::enqueue_user_capability_evolution(&conn, tool_name, outcome, summary)
            .map_err(|e| e.to_string())?;
    let _ = skilllite_evolution::log_evolution_event(
        &conn,
        &chat_root,
        "capability_evolution_authorized",
        tool_name,
        &format!("outcome={}, proposal_id={}", outcome, proposal_id),
        workspace,
    );
    // User-authorized proposals should be acted on promptly:
    // enqueue first (auditable), then trigger one immediate evolution run in background.
    let workspace_owned = workspace.to_string();
    let proposal_id_owned = proposal_id.clone();
    let skilllite_path_owned = skilllite_path.to_path_buf();
    let chat_root_for_log = chat_root.clone();
    std::thread::spawn(move || {
        let root = find_project_root(&workspace_owned);
        let mut cmd = std::process::Command::new(&skilllite_path_owned);
        cmd.arg("evolution")
            .arg("run")
            .arg("--json")
            .current_dir(&root)
            // Do NOT set SKILLLITE_WORKSPACE here: `agent-rpc` chat stores decisions under
            // `skilllite_core::paths::chat_root()` (default `~/.skilllite/chat` when workspace
            // env is unset). Forcing project root makes evolution open `<project>/chat`, a
            // different `feedback.sqlite` — empty decisions → "进化队列为空" and backlog mismatch.
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        for (k, v) in load_dotenv_for_child(&workspace_owned) {
            cmd.env(k, v);
        }
        // Must force this backlog row: generic `evolution run` only considers Passive/Active
        // proposals from build_evolution_proposals, not queued user-authorized capability rows.
        cmd.env("SKILLLITE_EVO_FORCE_PROPOSAL_ID", &proposal_id_owned);
        let output = cmd.output();
        let status_note = match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let mut merged = stdout.trim().to_string();
                if !stderr.trim().is_empty() {
                    if !merged.is_empty() {
                        merged.push_str(" | ");
                    }
                    merged.push_str(stderr.trim());
                }
                if merged.is_empty() {
                    format!("trigger_exit={}", out.status)
                } else {
                    let mut clipped = merged;
                    if clipped.len() > 280 {
                        clipped.truncate(280);
                        clipped.push('…');
                    }
                    clipped
                }
            }
            Err(e) => format!("trigger_failed: {}", e),
        };
        if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&chat_root_for_log) {
            let _ = skilllite_evolution::log_evolution_event(
                &conn,
                &chat_root_for_log,
                "capability_evolution_trigger_run",
                &proposal_id_owned,
                &status_note,
                &workspace_owned,
            );
        }
    });
    Ok(proposal_id)
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionProposalStatusDto {
    pub proposal_id: String,
    pub status: String,
    pub acceptance_status: String,
    pub updated_at: String,
    pub note: Option<String>,
}

pub fn get_evolution_proposal_status(
    _workspace: &str,
    proposal_id: &str,
) -> Result<EvolutionProposalStatusDto, String> {
    let chat_root = skilllite_core::paths::chat_root();
    let conn =
        skilllite_evolution::feedback::open_evolution_db(&chat_root).map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT proposal_id, status, acceptance_status, updated_at, note
         FROM evolution_backlog
         WHERE proposal_id = ?1
         LIMIT 1",
        [proposal_id],
        |row| {
            Ok(EvolutionProposalStatusDto {
                proposal_id: row.get(0)?,
                status: row.get(1)?,
                acceptance_status: row.get(2)?,
                updated_at: row.get(3)?,
                note: row.get(4)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionBacklogRowDto {
    pub proposal_id: String,
    pub source: String,
    pub risk_level: String,
    pub status: String,
    pub acceptance_status: String,
    pub roi_score: f64,
    pub updated_at: String,
    pub note: String,
}

pub fn load_evolution_backlog(_workspace: &str, limit: usize) -> Result<Vec<EvolutionBacklogRowDto>, String> {
    let chat_root = skilllite_core::paths::chat_root();
    let conn =
        skilllite_evolution::feedback::open_evolution_db(&chat_root).map_err(|e| e.to_string())?;
    let limit = limit.clamp(1, 200);
    let mut stmt = conn
        .prepare(
            "SELECT proposal_id, source, risk_level, status, acceptance_status, roi_score, updated_at, COALESCE(note, '')
             FROM evolution_backlog
             WHERE NOT (
               status = 'executed'
               AND COALESCE(acceptance_status, '') IN ('met', 'not_met')
             )
             ORDER BY updated_at DESC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit as i64], |row| {
            Ok(EvolutionBacklogRowDto {
                proposal_id: row.get(0)?,
                source: row.get(1)?,
                risk_level: row.get(2)?,
                status: row.get(3)?,
                acceptance_status: row.get(4)?,
                roi_score: row.get(5)?,
                updated_at: row.get(6)?,
                note: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;
    Ok(rows.flatten().collect())
}

pub fn trigger_evolution_run(
    workspace: &str,
    proposal_id: Option<&str>,
    _skilllite_path: &std::path::Path,
) -> Result<String, String> {
    fn env_first_non_empty(
        vars: &std::collections::HashMap<String, String>,
        keys: &[&str],
    ) -> Option<String> {
        for k in keys {
            if let Some(v) = vars.get(*k) {
                let t = v.trim();
                if !t.is_empty() {
                    return Some(t.to_string());
                }
            }
        }
        None
    }

    let root = find_project_root(workspace);
    let env_map: std::collections::HashMap<String, String> =
        load_dotenv_for_child(workspace).into_iter().collect();
    let api_base = env_first_non_empty(
        &env_map,
        &["SKILLLITE_API_BASE", "OPENAI_API_BASE", "OPENAI_BASE_URL", "BASE_URL"],
    )
    .or_else(|| skilllite_core::config::LlmConfig::try_from_env().map(|c| c.api_base))
    .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let api_key = env_first_non_empty(&env_map, &["SKILLLITE_API_KEY", "OPENAI_API_KEY", "API_KEY"])
        .or_else(|| skilllite_core::config::LlmConfig::try_from_env().map(|c| c.api_key))
        .unwrap_or_default();
    let model = env_first_non_empty(&env_map, &["SKILLLITE_MODEL", "OPENAI_MODEL", "MODEL"])
        .or_else(|| skilllite_core::config::LlmConfig::try_from_env().map(|c| c.model))
        .unwrap_or_else(|| "gpt-4o".to_string());

    if api_key.trim().is_empty() {
        return Err("执行 evolution run 失败: 缺少 API key（请配置 SKILLLITE_API_KEY 或 OPENAI_API_KEY）".to_string());
    }

    let llm = skilllite_agent::llm::LlmClient::new(&api_base, &api_key)
        .map_err(|e| format!("执行 evolution run 失败: 初始化 LLM 客户端失败: {}", e))?;
    let adapter = skilllite_agent::evolution::EvolutionLlmAdapter { llm: &llm };
    let skills_root = if root.join(".skills").is_dir() {
        Some(root.join(".skills"))
    } else if root.join("skills").is_dir() {
        Some(root.join("skills"))
    } else {
        None
    };

    let prev_force = std::env::var("SKILLLITE_EVO_FORCE_PROPOSAL_ID").ok();
    match proposal_id {
        Some(pid) => std::env::set_var("SKILLLITE_EVO_FORCE_PROPOSAL_ID", pid),
        None => std::env::remove_var("SKILLLITE_EVO_FORCE_PROPOSAL_ID"),
    }
    let run_result = tokio::runtime::Runtime::new()
        .map_err(|e| format!("执行 evolution run 失败: runtime 初始化失败: {}", e))?
        .block_on(skilllite_evolution::run_evolution(
            &skilllite_core::paths::chat_root(),
            skills_root.as_deref(),
            &adapter,
            &api_base,
            &api_key,
            &model,
            true,
        ));
    match prev_force {
        Some(v) => std::env::set_var("SKILLLITE_EVO_FORCE_PROPOSAL_ID", v),
        None => std::env::remove_var("SKILLLITE_EVO_FORCE_PROPOSAL_ID"),
    }

    let response = match run_result {
        Ok(skilllite_evolution::EvolutionRunResult::Completed(Some(txn_id))) => {
            let conn = skilllite_evolution::feedback::open_evolution_db(&skilllite_core::paths::chat_root())
                .map_err(|e| format!("执行 evolution run 失败: 打开进化数据库失败: {}", e))?;
            let changes = skilllite_evolution::query_changes_by_txn(&conn, &txn_id);
            let summary: Vec<String> = skilllite_evolution::format_evolution_changes(&changes);
            if summary.is_empty() {
                format!("Evolution completed (txn={})", txn_id)
            } else {
                summary.join("\n")
            }
        }
        Ok(skilllite_evolution::EvolutionRunResult::SkippedBusy) => {
            "Evolution skipped: another run in progress".to_string()
        }
        Ok(skilllite_evolution::EvolutionRunResult::NoScope)
        | Ok(skilllite_evolution::EvolutionRunResult::Completed(None)) => {
            let mut hint = String::from("Evolution: nothing to evolve");
            if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&skilllite_core::paths::chat_root()) {
                if let Ok((total, with_desc)) =
                    skilllite_evolution::feedback::count_decisions_with_task_desc(&conn)
                {
                    if total > 0 && with_desc == 0 {
                        hint.push_str("\n\n提示: 进化需要 task_description。当前未进化决策均无 task_description。");
                    } else if total == 0 {
                        let all_count: i64 = conn
                            .query_row("SELECT COUNT(*) FROM decisions", [], |r| r.get(0))
                            .unwrap_or(0);
                        if all_count > 0 {
                            hint.push_str("\n\n提示: 未进化决策队列为空。已有决策均已标记为已进化。");
                            hint.push_str("\n请执行新任务积累新决策后再触发进化。");
                        } else {
                            hint.push_str("\n\n提示: 进化队列为空。请先运行 skilllite run 或 skilllite chat 积累决策。");
                        }
                    }
                }
            }
            hint
        }
        Err(e) => return Err(format!("执行 evolution run 失败: {}", e)),
    };

    let mut clipped = response.clone();
    if clipped.len() > 480 {
        clipped.truncate(480);
        clipped.push('…');
    }
    if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&skilllite_core::paths::chat_root()) {
        let _ = skilllite_evolution::log_evolution_event(
            &conn,
            &skilllite_core::paths::chat_root(),
            "manual_evolution_run_triggered",
            proposal_id.unwrap_or("all"),
            &clipped,
            workspace,
        );
    }
    Ok(clipped)
}

// ─── Evolution diffs (prompt snapshot diff for UI) ────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionFileDiffDto {
    pub filename: String,
    pub evolved: bool,
    pub content: String,
    pub original_content: Option<String>,
}

fn evolved_prompt_files_from_changelog(chat_root: &std::path::Path) -> std::collections::HashSet<String> {
    let changelog = chat_root.join("prompts").join("_versions").join("changelog.jsonl");
    let mut evolved = std::collections::HashSet::new();
    if !changelog.exists() {
        return evolved;
    }
    let text = match std::fs::read_to_string(&changelog) {
        Ok(t) => t,
        Err(_) => return evolved,
    };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(files) = val.get("files").and_then(|f| f.as_array()) {
                for f in files {
                    if let Some(s) = f.as_str() {
                        evolved.insert(s.to_string());
                    }
                }
            }
        }
    }
    evolved
}

fn get_earliest_snapshot_content(chat_root: &std::path::Path, filename: &str) -> Option<String> {
    let versions_dir = chat_root.join("prompts").join("_versions");
    if !versions_dir.exists() {
        return None;
    }
    let mut txn_dirs: Vec<_> = std::fs::read_dir(&versions_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .collect();
    txn_dirs.sort_by_key(|e| e.file_name());
    for txn_dir in txn_dirs {
        let file_path = txn_dir.path().join(filename);
        if file_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&file_path) {
                return Some(content);
            }
        }
    }
    None
}

pub fn load_evolution_diffs(_workspace: &str) -> Vec<EvolutionFileDiffDto> {
    let chat_root = skilllite_core::paths::chat_root();
    let prompts_dir = chat_root.join("prompts");
    if !prompts_dir.exists() {
        return Vec::new();
    }
    let evolved_files = evolved_prompt_files_from_changelog(&chat_root);
    let prompt_files = [
        ("planning.md", "planning.md"),
        ("execution.md", "execution.md"),
        ("system.md", "system.md"),
        ("examples.md", "examples.md"),
        ("rules.json", "rules.json"),
        ("examples.json", "examples.json"),
    ];
    let mut result = Vec::new();
    for (_name, filename) in &prompt_files {
        let path = prompts_dir.join(filename);
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        if content.is_empty() && !evolved_files.contains(*filename) {
            continue;
        }
        let is_evolved = evolved_files.contains(*filename);
        let mut original_content: Option<String> = None;
        if is_evolved {
            original_content = get_earliest_snapshot_content(&chat_root, filename);
            if let Some(ref orig) = original_content {
                if orig == &content {
                    continue;
                }
            }
        }
        if !is_evolved {
            continue;
        }
        result.push(EvolutionFileDiffDto {
            filename: filename.to_string(),
            evolved: is_evolved,
            content,
            original_content,
        });
    }
    result
}

// ─── Onboarding: init workspace, probe Ollama ─────────────────────────────────

/// Requested provider during onboarding health check.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnboardingProvider {
    Api,
    Ollama,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HealthCheckItem {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OnboardingHealthCheckResult {
    pub binary: HealthCheckItem,
    pub provider: HealthCheckItem,
    pub workspace: HealthCheckItem,
    pub data_dir: HealthCheckItem,
    pub ok: bool,
}

pub fn run_onboarding_health_check(
    skilllite_path: &std::path::Path,
    workspace: &str,
    provider: OnboardingProvider,
    api_key: Option<&str>,
) -> OnboardingHealthCheckResult {
    let binary = check_bundled_skilllite(skilllite_path);
    let provider = check_provider(provider, api_key);
    let workspace = check_workspace(workspace);
    let data_dir = check_data_dir();
    let ok = binary.ok && provider.ok && workspace.ok && data_dir.ok;
    OnboardingHealthCheckResult {
        binary,
        provider,
        workspace,
        data_dir,
        ok,
    }
}

fn check_bundled_skilllite(skilllite_path: &std::path::Path) -> HealthCheckItem {
    if !skilllite_path.exists() {
        return HealthCheckItem {
            ok: false,
            message: format!("未找到 SkillLite 二进制：{}", skilllite_path.display()),
        };
    }

    match std::process::Command::new(skilllite_path)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) if status.success() => HealthCheckItem {
            ok: true,
            message: format!("内置引擎可用：{}", skilllite_path.display()),
        },
        Ok(status) => HealthCheckItem {
            ok: false,
            message: format!("内置引擎启动失败（状态：{}）", status),
        },
        Err(e) => HealthCheckItem {
            ok: false,
            message: format!("无法启动内置引擎：{}", e),
        },
    }
}

fn check_provider(provider: OnboardingProvider, api_key: Option<&str>) -> HealthCheckItem {
    match provider {
        OnboardingProvider::Api => {
            let has_key = api_key.map(|k| !k.trim().is_empty()).unwrap_or(false);
            if has_key {
                HealthCheckItem {
                    ok: true,
                    message: "已填写 API Key，可使用云模型".to_string(),
                }
            } else {
                HealthCheckItem {
                    ok: false,
                    message: "尚未填写 API Key".to_string(),
                }
            }
        }
        OnboardingProvider::Ollama => {
            let result = probe_ollama();
            if result.available && result.models.iter().any(|m| !m.contains("embed")) {
                HealthCheckItem {
                    ok: true,
                    message: format!("本机 Ollama 可用，检测到 {} 个模型", result.models.len()),
                }
            } else {
                HealthCheckItem {
                    ok: false,
                    message: "未检测到可用的 Ollama 聊天模型".to_string(),
                }
            }
        }
    }
}

fn check_workspace(workspace: &str) -> HealthCheckItem {
    let path = std::path::Path::new(workspace);
    if !path.exists() {
        return HealthCheckItem {
            ok: false,
            message: format!("工作区不存在：{}", path.display()),
        };
    }
    if !path.is_dir() {
        return HealthCheckItem {
            ok: false,
            message: format!("工作区不是目录：{}", path.display()),
        };
    }

    let probe_dir = path.join(".skilllite");
    match std::fs::create_dir_all(&probe_dir) {
        Ok(_) => HealthCheckItem {
            ok: true,
            message: format!("工作区可用：{}", path.display()),
        },
        Err(e) => HealthCheckItem {
            ok: false,
            message: format!("工作区不可写：{}", e),
        },
    }
}

fn check_data_dir() -> HealthCheckItem {
    let path = skilllite_core::paths::data_root();
    match std::fs::create_dir_all(&path) {
        Ok(_) => HealthCheckItem {
            ok: true,
            message: format!("数据目录可用：{}", path.display()),
        },
        Err(e) => HealthCheckItem {
            ok: false,
            message: format!("无法创建数据目录：{}", e),
        },
    }
}

/// Run `skilllite init` in the given directory. Creates .skills and example content.
pub fn init_workspace(dir: &str, skilllite_path: &std::path::Path) -> Result<(), String> {
    let path = std::path::Path::new(dir);
    if !path.is_dir() {
        return Err("目录不存在".to_string());
    }
    let mut cmd = std::process::Command::new(skilllite_path);
    cmd.arg("init").current_dir(path);
    let output = cmd
        .output()
        .map_err(|e| format!("执行 skilllite init 失败: {}", e))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(err.trim().to_string());
    }
    Ok(())
}

pub use skilllite_sandbox::{ProvisionRuntimesResult, RuntimeUiLine, RuntimeUiSnapshot};

/// Python/Node 来源探测（系统 PATH vs SkillLite 缓存下载），供左侧栏等 UI 展示。
pub fn probe_runtime_status() -> RuntimeUiSnapshot {
    skilllite_sandbox::probe_runtime_for_ui(None)
}

/// 预下载内置 Python/Node 运行时到缓存目录（`force` 时先删再下）。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProvisionProgressPayload {
    /// `"python"` | `"node"`
    pub phase: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<u32>,
}

fn parse_percent_from_runtime_progress_message(msg: &str) -> Option<u32> {
    for part in msg.split_whitespace() {
        if let Some(n) = part.strip_suffix('%') {
            if let Ok(p) = n.parse::<u32>() {
                return Some(p.min(100));
            }
        }
    }
    None
}

fn emit_runtime_provision_progress(app: &tauri::AppHandle, phase: &'static str, message: &str) {
    let percent = parse_percent_from_runtime_progress_message(message);
    let _ = app.emit(
        "skilllite-runtime-provision-progress",
        RuntimeProvisionProgressPayload {
            phase: phase.to_string(),
            message: message.to_string(),
            percent,
        },
    );
}

/// 无进度事件时使用（测试或脚本）；桌面端请用 [`provision_runtimes_with_emit`]。
#[allow(dead_code)]
pub fn provision_runtimes(python: bool, node: bool, force: bool) -> ProvisionRuntimesResult {
    skilllite_sandbox::provision_runtimes_to_cache(None, python, node, force, None, None)
}

/// 与 [`provision_runtimes`] 相同，但通过 `skilllite-runtime-provision-progress` 事件推送进度文案。
pub fn provision_runtimes_with_emit(
    app: &tauri::AppHandle,
    python: bool,
    node: bool,
    force: bool,
) -> ProvisionRuntimesResult {
    let py_progress = if python {
        let app = app.clone();
        Some(Box::new(move |m: &str| {
            emit_runtime_provision_progress(&app, "python", m);
        }) as Box<dyn Fn(&str) + Send>)
    } else {
        None
    };
    let node_progress = if node {
        let app = app.clone();
        Some(Box::new(move |m: &str| {
            emit_runtime_provision_progress(&app, "node", m);
        }) as Box<dyn Fn(&str) + Send>)
    } else {
        None
    };
    skilllite_sandbox::provision_runtimes_to_cache(
        None,
        python,
        node,
        force,
        py_progress,
        node_progress,
    )
}

/// Result of probing local Ollama (localhost:11434).
#[derive(Debug, Clone, serde::Serialize)]
pub struct OllamaProbeResult {
    pub available: bool,
    /// All installed model names.
    pub models: Vec<String>,
    /// Whether an embedding-capable model is present (name contains "embed").
    pub has_embedding: bool,
}

/// Probe Ollama at localhost:11434; returns availability, all model names, and embedding support.
pub fn probe_ollama() -> OllamaProbeResult {
    let empty = OllamaProbeResult {
        available: false,
        models: vec![],
        has_embedding: false,
    };
    let body = match ollama_get_tags() {
        Ok(b) => b,
        Err(_) => return empty,
    };
    let json: serde_json::Value = match serde_json::from_str(&body) {
        Ok(j) => j,
        Err(_) => return empty,
    };
    let arr = match json.get("models").and_then(|m| m.as_array()) {
        Some(a) => a,
        None => {
            return OllamaProbeResult {
                available: true,
                models: vec![],
                has_embedding: false,
            }
        }
    };
    let models: Vec<String> = arr
        .iter()
        .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
        .map(|s| s.to_string())
        .collect();
    let has_embedding = models.iter().any(|n| n.contains("embed"));
    OllamaProbeResult {
        available: true,
        models,
        has_embedding,
    }
}

fn ollama_get_tags() -> Result<String, ()> {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;

    let addr: SocketAddr = ([127, 0, 0, 1], 11434).into();
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|_| ())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| ())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .map_err(|_| ())?;

    let req = b"GET /api/tags HTTP/1.1\r\nHost: localhost:11434\r\nConnection: close\r\n\r\n";
    stream.write_all(req).map_err(|_| ())?;

    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).map_err(|_| ())?;
    let s = String::from_utf8_lossy(&buf);
    let body = s.split("\r\n\r\n").nth(1).unwrap_or("").trim();
    Ok(body.to_string())
}

/// Read `.skilllite/schedule.json` as pretty JSON; missing file → default `ScheduleFile`.
pub fn read_schedule_json(workspace: &str) -> Result<String, String> {
    use std::path::Path;
    let ws = Path::new(workspace);
    let sched = skilllite_core::schedule::load_schedule(ws)?;
    let file = sched.unwrap_or_default();
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

pub fn write_schedule_json(workspace: &str, json: &str) -> Result<(), String> {
    use std::path::Path;
    let file: skilllite_core::schedule::ScheduleFile =
        serde_json::from_str(json).map_err(|e| format!("schedule.json: {}", e))?;
    skilllite_core::schedule::save_schedule(Path::new(workspace), &file)
}
