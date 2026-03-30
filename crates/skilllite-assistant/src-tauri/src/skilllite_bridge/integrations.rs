//! 技能、进化、引导、运行时、Ollama、日程。

use serde::Serialize;
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
