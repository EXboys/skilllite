//! 技能、进化、引导、运行时、Ollama、日程。

use serde::Serialize;
use skilllite_core::skill::{
    discovery::{discover_skill_instances_in_workspace, resolve_skills_dir_with_legacy_fallback},
    manifest,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tauri::Emitter;

use super::chat::{merge_dotenv_with_chat_overrides, ChatConfigOverrides};
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

fn discover_scripted_skill_instances(root: &std::path::Path) -> Vec<(PathBuf, String)> {
    discover_skill_instances_in_workspace(root, None)
        .into_iter()
        .filter(|skill| skill_has_scripts(&skill.path))
        .map(|skill| (skill.path, skill.name))
        .collect()
}

fn resolve_workspace_skills_root(workspace: &str) -> PathBuf {
    let root = find_project_root(workspace);
    resolve_skills_dir_with_legacy_fallback(&root, "skills").effective_path
}

fn existing_workspace_skills_root(workspace: &str) -> Option<PathBuf> {
    let skills_root = resolve_workspace_skills_root(workspace);
    skills_root.is_dir().then_some(skills_root)
}

/// List skill names in workspace (for repair UI) using core-owned discovery.
pub fn list_skill_names(workspace: &str) -> Vec<String> {
    let root = find_project_root(workspace);
    let mut names = std::collections::HashSet::new();
    for (_, name) in discover_scripted_skill_instances(&root) {
        names.insert(name);
    }
    let mut v: Vec<String> = names.into_iter().collect();
    v.sort();
    v
}

/// Resolve skill directory path by name using core-owned discovery.
fn find_skill_dir(workspace: &str, skill_name: &str) -> Option<std::path::PathBuf> {
    let root = find_project_root(workspace);
    for (path, name) in discover_scripted_skill_instances(&root) {
        if name == skill_name {
            return Some(path);
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

/// Remove installed skills under the workspace (same discovery as list/open).
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
        fs::remove_dir_all(&skill_path)
            .map_err(|e| format!("删除目录失败 {}: {}", skill_path.display(), e))?;
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

/// Run `skilllite add <source>` in the workspace using the canonical resolved skills dir.
/// Source: owner/repo, owner/repo@skill-name, https://github.com/..., or local path.
pub fn add_skill(
    workspace: &str,
    source: &str,
    force: bool,
    skilllite_path: &std::path::Path,
) -> Result<String, String> {
    let root = find_project_root(workspace);
    let skills_root = resolve_workspace_skills_root(workspace);
    let source = source.trim();
    if source.is_empty() {
        return Err("请填写来源，例如：owner/repo 或 owner/repo@skill-name".to_string());
    }

    let mut cmd = std::process::Command::new(skilllite_path);
    cmd.arg("add")
        .arg(source)
        .arg("--skills-dir")
        .arg(&skills_root);
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

fn is_evolution_runtime_env_key(k: &str) -> bool {
    use skilllite_core::config::env_keys::evolution as evo;
    k == evo::SKILLLITE_EVOLUTION
        || k == evo::SKILLLITE_MAX_EVOLUTIONS_PER_DAY
        || k.starts_with("SKILLLITE_EVO")
        || k.starts_with("SKILLLITE_EVOLUTION_")
}

/// 将合并后的子进程环境变量中、与进化运行时相关的键同步到当前进程，供 `run_evolution` 内 `from_env()` 读取。
struct EvolutionRunEnvGuard {
    prev: Vec<(String, Option<String>)>,
}

impl EvolutionRunEnvGuard {
    fn push_from_merged(m: &std::collections::HashMap<String, String>) -> Self {
        let mut prev = Vec::new();
        for (k, v) in m.iter() {
            if !is_evolution_runtime_env_key(k) {
                continue;
            }
            prev.push((k.clone(), std::env::var(k).ok()));
            std::env::set_var(k, v);
        }
        Self { prev }
    }
}

impl Drop for EvolutionRunEnvGuard {
    fn drop(&mut self) {
        for (k, ov) in self.prev.drain(..) {
            match ov {
                Some(val) => std::env::set_var(&k, val),
                None => std::env::remove_var(&k),
            }
        }
    }
}

fn effective_evolution_interval_secs(workspace: &str, cfg: Option<&ChatConfigOverrides>) -> u64 {
    use skilllite_core::config::env_keys::evolution as evo_env;
    if let Some(c) = cfg {
        if let Some(n) = c.evolution_interval_secs.filter(|&n| n > 0) {
            return n;
        }
    }
    workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION_INTERVAL_SECS)
        .and_then(|v| v.parse().ok())
        .unwrap_or(600)
}

fn effective_evolution_decision_threshold(
    workspace: &str,
    cfg: Option<&ChatConfigOverrides>,
) -> i64 {
    use skilllite_core::config::env_keys::evolution as evo_env;
    if let Some(c) = cfg {
        if let Some(n) = c.evolution_decision_threshold.filter(|&n| n > 0) {
            return i64::from(n);
        }
    }
    workspace_env_lookup(workspace, evo_env::SKILLLITE_EVOLUTION_DECISION_THRESHOLD)
        .and_then(|v| v.parse().ok())
        .unwrap_or(10)
}

fn effective_evo_profile_key(workspace: &str, cfg: Option<&ChatConfigOverrides>) -> &'static str {
    use skilllite_core::config::env_keys::evolution as evo_env;
    if let Some(c) = cfg {
        if let Some(ref p) = c.evo_profile {
            let t = p.trim();
            if t == "demo" {
                return "demo";
            }
            if t == "conservative" {
                return "conservative";
            }
        }
    }
    match workspace_env_lookup(workspace, evo_env::SKILLLITE_EVO_PROFILE).as_deref() {
        Some("demo") => "demo",
        Some("conservative") => "conservative",
        _ => "default",
    }
}

fn effective_evo_cooldown_hours(workspace: &str, cfg: Option<&ChatConfigOverrides>) -> f64 {
    use skilllite_core::config::env_keys::evolution as evo_env;
    if let Some(c) = cfg {
        if let Some(h) = c.evo_cooldown_hours.filter(|h| h.is_finite() && *h >= 0.0) {
            return h;
        }
    }
    workspace_env_lookup(workspace, evo_env::SKILLLITE_EVO_COOLDOWN_HOURS)
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| skilllite_evolution::EvolutionThresholds::default().cooldown_hours)
}

/// Merged A9 growth config: workspace `.env` + optional UI overrides for interval / raw threshold.
fn growth_schedule_merged_for_workspace(
    workspace: &str,
    cfg: Option<&ChatConfigOverrides>,
) -> skilllite_evolution::growth_schedule::GrowthScheduleConfig {
    let mut c = skilllite_evolution::growth_schedule::GrowthScheduleConfig::from_env();
    c.interval_secs = effective_evolution_interval_secs(workspace, cfg);
    c.raw_unprocessed_threshold = effective_evolution_decision_threshold(workspace, cfg);
    c
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
/// A9: periodic interval **or** weighted recent signals **or** raw unprocessed count **or** long-interval sweep.
/// When **only** the periodic arm is due and no proposals would be built, returns `false` (no subprocess,
/// no `evolution_run_outcome` spam). Signal/sweep arms still spawn even if proposals end up empty.
/// Merged workspace `.env` + UI overrides are applied for the preflight check so thresholds match the child.
///
/// `last_periodic_spawn_unix`: last time the **periodic** arm fired; updated when the periodic
/// condition is met. Initialized lazily on first check so the first periodic window starts then.
pub fn evolution_growth_due(
    workspace: &str,
    last_periodic_spawn_unix: &std::sync::Mutex<Option<i64>>,
    cfg: Option<&ChatConfigOverrides>,
) -> bool {
    let mode = evolution_mode_from_workspace(workspace);
    if mode.is_disabled() {
        return false;
    }
    let schedule_cfg = growth_schedule_merged_for_workspace(workspace, cfg);
    let chat_root = skilllite_core::paths::chat_root();
    let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&chat_root) else {
        return false;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let mut g = last_periodic_spawn_unix
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let outcome =
        skilllite_evolution::growth_schedule::growth_due(&conn, now, &mut g, &schedule_cfg)
            .unwrap_or_default();

    if !outcome.due {
        return false;
    }
    if outcome.periodic_only {
        let dotenv = load_dotenv_for_child(workspace);
        let merged_vec = merge_dotenv_with_chat_overrides(dotenv, cfg);
        let merged_map: HashMap<String, String> = merged_vec.into_iter().collect();
        let _guard = EvolutionRunEnvGuard::push_from_merged(&merged_map);
        return skilllite_evolution::would_have_evolution_proposals(
            &conn,
            skilllite_evolution::EvolutionMode::from_env(),
            false,
        )
        .unwrap_or(true);
    }
    true
}

fn evolution_mode_labels(
    mode: &skilllite_evolution::EvolutionMode,
) -> (&'static str, &'static str) {
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
    /// `evolution_log.version`（常为本轮 txn，如 `evo_YYYYMMDD_HHMMSS`）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub txn_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvolutionStatusPayload {
    pub mode_key: String,
    pub mode_label: String,
    pub interval_secs: u64,
    pub decision_threshold: i64,
    /// Weighted sum over the latest `signal_window` meaningful unprocessed decisions (A9).
    pub weighted_signal_sum: i64,
    /// Env `SKILLLITE_EVO_TRIGGER_WEIGHTED_MIN` (after workspace merge for interval/threshold only).
    pub weighted_trigger_min: i64,
    pub signal_window: i64,
    /// `default` / `demo` / `conservative`（应用内覆盖优先于工作区 `.env`）。
    pub evo_profile_key: String,
    pub evo_cooldown_hours: f64,
    pub unprocessed_decisions: i64,
    pub last_run_ts: Option<String>,
    pub judgement_label: Option<String>,
    pub judgement_reason: Option<String>,
    pub recent_events: Vec<EvolutionLogEntryDto>,
    pub pending_skill_count: usize,
    pub db_error: Option<String>,
}

/// Evolution feedback DB + schedule hints for the assistant UI.
pub fn load_evolution_status(
    workspace: &str,
    cfg: Option<ChatConfigOverrides>,
) -> EvolutionStatusPayload {
    let mode = evolution_mode_from_workspace(workspace);
    let (mode_key, mode_label) = evolution_mode_labels(&mode);

    let schedule_cfg = growth_schedule_merged_for_workspace(workspace, cfg.as_ref());
    let interval_secs = schedule_cfg.interval_secs;
    let decision_threshold = schedule_cfg.raw_unprocessed_threshold;
    let weighted_trigger_min = schedule_cfg.weighted_min;
    let signal_window = schedule_cfg.signal_window;
    let evo_profile_key = effective_evo_profile_key(workspace, cfg.as_ref()).to_string();
    let evo_cooldown_hours = effective_evo_cooldown_hours(workspace, cfg.as_ref());

    let chat_root = skilllite_core::paths::chat_root();
    let mut pending_skill_count = 0;
    if let Some(skills_root) = existing_workspace_skills_root(workspace) {
        pending_skill_count =
            skilllite_evolution::skill_synth::list_pending_skills_with_review(&skills_root).len();
    }

    let mut db_error = None;
    let mut unprocessed_decisions = 0i64;
    let mut weighted_signal_sum = 0i64;
    let mut recent_events = Vec::new();
    let mut last_run_ts = None;
    let mut judgement_label = None;
    let mut judgement_reason = None;

    match skilllite_evolution::feedback::open_evolution_db(&chat_root) {
        Ok(conn) => {
            if let Ok(c) = skilllite_evolution::feedback::count_unprocessed_decisions(&conn) {
                unprocessed_decisions = c;
            }
            if let Ok(w) = skilllite_evolution::growth_schedule::weighted_unprocessed_signal_sum(
                &conn,
                signal_window,
            ) {
                weighted_signal_sum = w;
            }
            if let Ok(Some(summary)) = skilllite_evolution::feedback::build_latest_judgement(&conn)
            {
                judgement_label = Some(summary.judgement.label_zh().to_string());
                judgement_reason = Some(summary.reason);
            }
            // Last UI "attempt" timestamp: material run or no-output run (constants are fixed literals).
            let last_attempt_sql = format!(
                "SELECT ts FROM evolution_log WHERE type IN ('{}', '{}') ORDER BY ts DESC LIMIT 1",
                skilllite_evolution::feedback::EVOLUTION_LOG_TYPE_RUN_MATERIAL,
                skilllite_evolution::feedback::EVOLUTION_LOG_TYPE_RUN_NOOP,
            );
            if let Ok(mut stmt) = conn.prepare(&last_attempt_sql) {
                if let Ok(mut rows) = stmt.query([]) {
                    if let Ok(Some(row)) = rows.next() {
                        last_run_ts = row.get(0).ok();
                    }
                }
            }
            if let Ok(mut stmt) = conn.prepare(
                "SELECT ts, type, target_id, reason, version FROM evolution_log ORDER BY ts DESC LIMIT 25",
            ) {
                if let Ok(rows) = stmt.query_map([], |row| {
                    let version: Option<String> = row.get(4)?;
                    let txn_id = version
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.trim().to_string());
                    Ok(EvolutionLogEntryDto {
                        ts: row.get(0)?,
                        event_type: row.get(1)?,
                        target_id: row.get::<_, Option<String>>(2)?,
                        reason: row.get::<_, Option<String>>(3)?,
                        txn_id,
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
        weighted_signal_sum,
        weighted_trigger_min,
        signal_window,
        evo_profile_key,
        evo_cooldown_hours,
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
    let Some(skills_root) = existing_workspace_skills_root(workspace) else {
        return Vec::new();
    };
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

pub fn read_evolution_pending_skill_md(
    workspace: &str,
    skill_name: &str,
) -> Result<String, String> {
    let skills_root = resolve_workspace_skills_root(workspace);
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
    let skills_root = resolve_workspace_skills_root(workspace);
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
    let skills_root = resolve_workspace_skills_root(workspace);
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

pub fn load_evolution_backlog(
    _workspace: &str,
    limit: usize,
) -> Result<Vec<EvolutionBacklogRowDto>, String> {
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
    overrides: Option<ChatConfigOverrides>,
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

    let env_map: std::collections::HashMap<String, String> =
        load_dotenv_for_child(workspace).into_iter().collect();
    let mut api_base = env_first_non_empty(
        &env_map,
        &[
            "SKILLLITE_API_BASE",
            "OPENAI_API_BASE",
            "OPENAI_BASE_URL",
            "BASE_URL",
        ],
    )
    .or_else(|| skilllite_core::config::LlmConfig::try_from_env().map(|c| c.api_base))
    .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
    let mut api_key = env_first_non_empty(
        &env_map,
        &["SKILLLITE_API_KEY", "OPENAI_API_KEY", "API_KEY"],
    )
    .or_else(|| skilllite_core::config::LlmConfig::try_from_env().map(|c| c.api_key))
    .unwrap_or_default();
    let mut model = env_first_non_empty(&env_map, &["SKILLLITE_MODEL", "OPENAI_MODEL", "MODEL"])
        .or_else(|| skilllite_core::config::LlmConfig::try_from_env().map(|c| c.model))
        .unwrap_or_else(|| "gpt-4o".to_string());

    // Match chat: keys/base/model from Settings are not written to process env; merge here so
    // manual evolution trigger works when the user only configured the assistant UI.
    if let Some(ref cfg) = overrides {
        if let Some(ref b) = cfg.api_base {
            if !b.trim().is_empty() {
                api_base = b.clone();
            }
        }
        if let Some(ref k) = cfg.api_key {
            if !k.trim().is_empty() {
                api_key = k.clone();
            }
        }
        if let Some(ref m) = cfg.model {
            if !m.trim().is_empty() {
                model = m.clone();
            }
        }
    }

    if api_key.trim().is_empty() {
        return Err(
            "执行 evolution run 失败: 缺少 API key（请配置 SKILLLITE_API_KEY 或 OPENAI_API_KEY）"
                .to_string(),
        );
    }

    let dotenv = load_dotenv_for_child(workspace);
    let merged_vec = merge_dotenv_with_chat_overrides(dotenv, overrides.as_ref());
    let merged_map: std::collections::HashMap<String, String> = merged_vec.into_iter().collect();
    let _evo_env_guard = EvolutionRunEnvGuard::push_from_merged(&merged_map);

    let llm = skilllite_agent::llm::LlmClient::new(&api_base, &api_key)
        .map_err(|e| format!("执行 evolution run 失败: 初始化 LLM 客户端失败: {}", e))?;
    let adapter = skilllite_agent::evolution::EvolutionLlmAdapter { llm: &llm };
    let skills_root = existing_workspace_skills_root(workspace);

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
            let conn = skilllite_evolution::feedback::open_evolution_db(
                &skilllite_core::paths::chat_root(),
            )
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
            if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(
                &skilllite_core::paths::chat_root(),
            ) {
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
                            hint.push_str(
                                "\n\n提示: 未进化决策队列为空。已有决策均已标记为已进化。",
                            );
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
    if let Ok(conn) =
        skilllite_evolution::feedback::open_evolution_db(&skilllite_core::paths::chat_root())
    {
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

/// One evolution txn directory under `prompts/_versions/<txn_id>/` that contains a prompt file.
#[derive(Debug, Clone, Serialize)]
pub struct EvolutionSnapshotTxnDto {
    pub txn_id: String,
    /// Best-effort mtime of the snapshot file (seconds since UNIX epoch).
    pub modified_unix: i64,
}

/// Read live `prompts/<filename>` (not a txn snapshot). Used by the assistant UI version picker.
pub const PROMPT_VERSION_CURRENT: &str = "__current__";

const MAX_PROMPT_VERSION_BYTES: u64 = 2 * 1024 * 1024;

const EVOLUTION_PROMPT_DIFF_FILENAMES: &[&str] = &[
    "planning.md",
    "execution.md",
    "system.md",
    "examples.md",
    "rules.json",
    "examples.json",
];

fn evolution_prompt_filename_allowed(name: &str) -> bool {
    EVOLUTION_PROMPT_DIFF_FILENAMES.contains(&name)
}

fn safe_evolution_txn_dir_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 256 {
        return false;
    }
    if name == PROMPT_VERSION_CURRENT {
        return false;
    }
    if name.contains("..") {
        return false;
    }
    name.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn read_utf8_file_capped(path: &std::path::Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        return Err("路径是目录".to_string());
    }
    let len = meta.len();
    if len > MAX_PROMPT_VERSION_BYTES {
        return Err(format!(
            "文件超过 {} 字节上限，无法在应用内对比",
            MAX_PROMPT_VERSION_BYTES
        ));
    }
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

/// List txn snapshot dirs that contain `filename`, newest first (by snapshot file mtime).
pub fn list_prompt_snapshot_txns_at(
    chat_root: &std::path::Path,
    filename: &str,
) -> Result<Vec<EvolutionSnapshotTxnDto>, String> {
    if !evolution_prompt_filename_allowed(filename) {
        return Err("不支持的 prompt 文件名".to_string());
    }
    let versions_dir = chat_root.join("prompts").join("_versions");
    if !versions_dir.is_dir() {
        return Ok(Vec::new());
    }
    let entries = std::fs::read_dir(&versions_dir).map_err(|e| e.to_string())?;
    let mut out: Vec<EvolutionSnapshotTxnDto> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        if !file_type.is_dir() {
            continue;
        }
        let txn_id = entry.file_name().to_string_lossy().into_owned();
        if !safe_evolution_txn_dir_name(&txn_id) {
            continue;
        }
        let snap_path = entry.path().join(filename);
        if !snap_path.is_file() {
            continue;
        }
        let modified_unix = std::fs::metadata(&snap_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        out.push(EvolutionSnapshotTxnDto {
            txn_id,
            modified_unix,
        });
    }
    out.sort_by_key(|t| std::cmp::Reverse(t.modified_unix));
    Ok(out)
}

/// Read one prompt `filename` from either live prompts or `_versions/<txn_id>/`.
pub fn read_prompt_snapshot_version_at(
    chat_root: &std::path::Path,
    filename: &str,
    version_ref: &str,
) -> Result<String, String> {
    if !evolution_prompt_filename_allowed(filename) {
        return Err("不支持的 prompt 文件名".to_string());
    }
    let trimmed = version_ref.trim();
    if trimmed == PROMPT_VERSION_CURRENT {
        let path = chat_root.join("prompts").join(filename);
        if !path.is_file() {
            return Ok(String::new());
        }
        return read_utf8_file_capped(&path);
    }
    if !safe_evolution_txn_dir_name(trimmed) {
        return Err("无效的版本标识".to_string());
    }
    let path = chat_root
        .join("prompts")
        .join("_versions")
        .join(trimmed)
        .join(filename);
    if !path.is_file() {
        return Err("该快照中无此文件".to_string());
    }
    read_utf8_file_capped(&path)
}

pub fn list_prompt_snapshot_txns(filename: &str) -> Result<Vec<EvolutionSnapshotTxnDto>, String> {
    list_prompt_snapshot_txns_at(&skilllite_core::paths::chat_root(), filename)
}

fn list_prompt_snapshots_batch_at(
    chat_root: &std::path::Path,
    filenames: &[String],
) -> Result<HashMap<String, Vec<EvolutionSnapshotTxnDto>>, String> {
    let mut out = HashMap::new();
    for name in filenames {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !evolution_prompt_filename_allowed(trimmed) {
            return Err(format!("不支持的 prompt 文件名: {}", trimmed));
        }
        let list = list_prompt_snapshot_txns_at(chat_root, trimmed)?;
        out.insert(trimmed.to_string(), list);
    }
    Ok(out)
}

/// 一次请求列出多个 prompt 文件的快照 txn（避免前端 N 路并行 invoke 与 Strict Mode 竞态）。
pub fn list_prompt_snapshots_batch(
    filenames: &[String],
) -> Result<HashMap<String, Vec<EvolutionSnapshotTxnDto>>, String> {
    list_prompt_snapshots_batch_at(&skilllite_core::paths::chat_root(), filenames)
}

pub fn read_prompt_version_content(filename: &str, version_ref: &str) -> Result<String, String> {
    read_prompt_snapshot_version_at(&skilllite_core::paths::chat_root(), filename, version_ref)
}

/// Write UTF-8 to `chat_root/prompts/<filename>`（仅允许与快照对比相同白名单）。
pub fn write_chat_prompt_text_file(filename: &str, content: &str) -> Result<(), String> {
    if !evolution_prompt_filename_allowed(filename) {
        return Err("不支持的 prompt 文件名".to_string());
    }
    let len = content.len() as u64;
    if len > MAX_PROMPT_VERSION_BYTES {
        return Err(format!("内容超过 {} 字节上限", MAX_PROMPT_VERSION_BYTES));
    }
    let path = skilllite_core::paths::chat_root()
        .join("prompts")
        .join(filename);
    skilllite_fs::write_file(&path, content).map_err(|e| e.to_string())
}

fn evolved_prompt_files_from_changelog(
    chat_root: &std::path::Path,
) -> std::collections::HashSet<String> {
    let changelog = chat_root
        .join("prompts")
        .join("_versions")
        .join("changelog.jsonl");
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
        let original_content = if is_evolved {
            get_earliest_snapshot_content(&chat_root, filename)
        } else {
            None
        };
        result.push(EvolutionFileDiffDto {
            filename: filename.to_string(),
            evolved: is_evolved,
            content,
            original_content,
        });
    }
    result
}

#[cfg(test)]
mod skill_discovery_tests {
    use super::*;

    fn temp_test_dir(prefix: &str) -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("duration")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "skilllite_assistant_{}_{}_{}",
            prefix,
            std::process::id(),
            unique
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn list_skill_names_uses_core_discovery_roots() {
        let tmp = temp_test_dir("skill_roots");
        let nested_skill = tmp.join(".claude").join("skills").join("nested-skill");
        std::fs::create_dir_all(nested_skill.join("scripts")).expect("nested scripts");
        std::fs::write(nested_skill.join("SKILL.md"), "name: nested-skill\n").expect("nested md");
        std::fs::write(
            nested_skill.join("scripts").join("run.sh"),
            "#!/usr/bin/env bash\necho ok\n",
        )
        .expect("nested script");

        let evolved_skill = tmp.join(".skills").join("_evolved").join("evolved-skill");
        std::fs::create_dir_all(evolved_skill.join("scripts")).expect("evolved scripts");
        std::fs::write(evolved_skill.join("SKILL.md"), "name: evolved-skill\n").expect("evolved md");
        std::fs::write(
            evolved_skill.join("scripts").join("run.py"),
            "print('ok')\n",
        )
        .expect("evolved script");

        let names = list_skill_names(nested_skill.to_string_lossy().as_ref());
        assert_eq!(names, vec!["evolved-skill", "nested-skill"]);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resolve_workspace_skills_root_keeps_legacy_fallback() {
        let tmp = temp_test_dir("legacy_fallback");
        let legacy = tmp.join(".skills");
        std::fs::create_dir_all(&legacy).expect("legacy root");

        let resolved = resolve_workspace_skills_root(tmp.to_string_lossy().as_ref());
        assert_eq!(
            resolved.canonicalize().expect("resolved canonical"),
            legacy.canonicalize().expect("legacy canonical")
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }
}

#[cfg(test)]
mod evolution_prompt_version_tests {
    use super::*;

    #[test]
    fn list_txns_newest_first_and_read_roundtrip() {
        let root = std::env::temp_dir().join(format!("sl_evo_txn_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(
            root.join("prompts")
                .join("_versions")
                .join("evo_20260101_000000"),
        )
        .expect("mkdir txn0");
        std::fs::write(
            root.join("prompts")
                .join("_versions")
                .join("evo_20260101_000000")
                .join("rules.json"),
            b"v1",
        )
        .expect("write v1");
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::create_dir_all(
            root.join("prompts")
                .join("_versions")
                .join("evo_20260101_000001"),
        )
        .expect("mkdir txn1");
        std::fs::write(
            root.join("prompts")
                .join("_versions")
                .join("evo_20260101_000001")
                .join("rules.json"),
            b"v2",
        )
        .expect("write v2");
        std::fs::write(root.join("prompts").join("rules.json"), b"live").expect("write live");

        let list = list_prompt_snapshot_txns_at(&root, "rules.json").expect("list");
        assert_eq!(list.len(), 2);
        assert!(
            list[0].modified_unix >= list[1].modified_unix,
            "expect newest txn first"
        );
        let ids: std::collections::HashSet<&str> = list.iter().map(|t| t.txn_id.as_str()).collect();
        assert!(ids.contains("evo_20260101_000001"));
        assert!(ids.contains("evo_20260101_000000"));

        assert_eq!(
            read_prompt_snapshot_version_at(&root, "rules.json", PROMPT_VERSION_CURRENT)
                .expect("cur"),
            "live"
        );
        assert_eq!(
            read_prompt_snapshot_version_at(&root, "rules.json", "evo_20260101_000000")
                .expect("t0"),
            "v1"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_disallowed_filename() {
        let root = std::path::Path::new("/");
        let r = list_prompt_snapshot_txns_at(root, "../secrets");
        assert!(r.is_err());
    }

    #[test]
    fn write_chat_prompt_rejects_bad_filename() {
        let r = super::write_chat_prompt_text_file("../../../etc/passwd", "x");
        assert!(r.is_err());
    }

    #[test]
    fn batch_lists_multiple_files() {
        let root = std::env::temp_dir().join(format!("sl_evo_batch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(
            root.join("prompts")
                .join("_versions")
                .join("evo_20990101_000000"),
        )
        .expect("mkdir");
        std::fs::write(
            root.join("prompts")
                .join("_versions")
                .join("evo_20990101_000000")
                .join("rules.json"),
            b"{}",
        )
        .expect("write rules");
        std::fs::write(
            root.join("prompts")
                .join("_versions")
                .join("evo_20990101_000000")
                .join("examples.json"),
            b"[]",
        )
        .expect("write examples");
        let names = vec!["rules.json".to_string(), "examples.json".to_string()];
        let map = super::list_prompt_snapshots_batch_at(&root, &names).expect("batch");
        assert_eq!(map.len(), 2);
        assert_eq!(map.get("rules.json").map(|v| v.len()), Some(1));
        let _ = std::fs::remove_dir_all(&root);
    }
}

// ─── Onboarding: init workspace, probe Ollama ─────────────────────────────────

/// Requested provider during onboarding health check.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OnboardingProvider {
    Api,
    Ollama,
}

/// 反序列化时接受 `apiKey`（Tauri 与 Rust `api_key` 的约定映射）以及历史误用的 `api_key`。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingHealthCheckInput {
    pub workspace: String,
    pub provider: OnboardingProvider,
    #[serde(default, alias = "api_key")]
    pub api_key: Option<String>,
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
    let trimmed = dir.trim();
    if trimmed.is_empty() {
        return Err("工作区路径为空".to_string());
    }
    let path = std::path::Path::new(trimmed);
    if !path.is_absolute() {
        return Err(
            "初始化技能需要工作区的绝对路径。请在「设置 → 工作区」点击「浏览」选择项目文件夹；\
             不要将路径设为「.」或相对路径。（桌面应用进程的工作目录常为 /，使用「.」会在 /skills 创建目录并失败。）"
                .to_string(),
        );
    }
    if !path.is_dir() {
        return Err("目录不存在".to_string());
    }
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if path == std::path::Path::new("/") {
        return Err("不能使用根目录 / 作为工作区".to_string());
    }
    let mut cmd = std::process::Command::new(skilllite_path);
    cmd.arg("init").current_dir(&path);
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

#[cfg(test)]
mod onboarding_health_check_input_tests {
    use super::OnboardingHealthCheckInput;

    #[test]
    fn deserializes_api_key_camel_case() {
        let j = r#"{"workspace":"/tmp/ws","provider":"api","apiKey":"sk-test"}"#;
        let v: OnboardingHealthCheckInput = serde_json::from_str(j).unwrap();
        assert_eq!(v.workspace, "/tmp/ws");
        assert_eq!(v.api_key.as_deref(), Some("sk-test"));
    }

    #[test]
    fn deserializes_api_key_snake_case_alias() {
        let j = r#"{"workspace":"/tmp/ws","provider":"api","api_key":"sk-legacy"}"#;
        let v: OnboardingHealthCheckInput = serde_json::from_str(j).unwrap();
        assert_eq!(v.api_key.as_deref(), Some("sk-legacy"));
    }
}
