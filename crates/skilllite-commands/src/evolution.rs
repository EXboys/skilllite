//! EVO-5: Evolution management CLI commands.
//!
//! Provides `skilllite evolution {status,reset,disable,explain,run}` subcommands
//! for inspecting, controlling, and debugging the self-evolution engine.

use anyhow::Context;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use skilllite_agent::types::AgentConfig;

use crate::error::bail;
use crate::Result;
use skilllite_core::config::env_keys::paths as env_paths;
use skilllite_core::paths;
use skilllite_core::protocol::{NewSkill, NodeResult};
use skilllite_core::skill::manifest;

/// Resolve workspace for project-level skill evolution.
/// Uses SKILLLITE_WORKSPACE env or current_dir. Returns workspace/.skills.
fn resolve_skills_root(workspace: Option<&str>) -> Option<PathBuf> {
    let ws: PathBuf = workspace
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var(env_paths::SKILLLITE_WORKSPACE)
                .ok()
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let ws = if ws.is_absolute() {
        ws
    } else {
        std::env::current_dir().ok()?.join(ws)
    };
    Some(ws.join(".skills"))
}

/// `skilllite evolution status` — show evolution statistics, effectiveness, trends.
pub fn cmd_status() -> Result<()> {
    let root = paths::chat_root();
    let conn = skilllite_evolution::feedback::open_evolution_db(&root)?;
    let mode = skilllite_evolution::EvolutionMode::from_env();

    // Header
    println!("╭─────────────────────────────────────────────╮");
    println!("│       SkillLite 自进化引擎状态               │");
    println!("╰─────────────────────────────────────────────╯");
    println!();

    // Mode
    let mode_str = match &mode {
        skilllite_evolution::EvolutionMode::All => "全部启用 ✅",
        skilllite_evolution::EvolutionMode::PromptsOnly => "仅 Prompts",
        skilllite_evolution::EvolutionMode::MemoryOnly => "仅 Memory",
        skilllite_evolution::EvolutionMode::SkillsOnly => "仅 Skills",
        skilllite_evolution::EvolutionMode::Disabled => "已禁用 ⏸️  (已有进化产物冻结生效中)",
    };
    println!("进化模式: {}", mode_str);
    println!();

    // Recent metrics trend
    println!("📈 核心指标趋势 (最近 7 天)");
    println!(
        "  {:10} {:>8} {:>8} {:>8}",
        "日期", "成功率", "Replan", "纠正率"
    );
    println!(
        "  {:10} {:>8} {:>8} {:>8}",
        "──────────", "────────", "────────", "────────"
    );

    let mut stmt = conn
        .prepare(
            "SELECT date, first_success_rate, avg_replans, user_correction_rate
         FROM evolution_metrics
         WHERE date > date('now', '-7 days') ORDER BY date DESC",
        )
        .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
    let metrics = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, f64>(1)?,
                row.get::<_, f64>(2)?,
                row.get::<_, f64>(3)?,
            ))
        })
        .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;

    let mut has_metrics = false;
    for m in metrics {
        let (date, fsr, avg_r, ucr) = m.map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
        println!(
            "  {:10} {:>7.0}% {:>8.1} {:>7.0}%",
            date,
            fsr * 100.0,
            avg_r,
            ucr * 100.0,
        );
        has_metrics = true;
    }
    if !has_metrics {
        println!("  (暂无数据 — 需要更多使用后才会出现)");
    }
    if let Ok(Some(summary)) = skilllite_evolution::feedback::build_latest_judgement(&conn) {
        println!(
            "  — 当前简单判断: {} ({})",
            summary.judgement.label_zh(),
            summary.judgement.as_str()
        );
        println!("    原因: {}", summary.reason);
    }
    println!();

    // Recent evolution events
    println!("📜 最近进化事件");
    let mut stmt = conn
        .prepare(
            "SELECT ts, type, target_id, reason FROM evolution_log
         ORDER BY ts DESC LIMIT 10",
        )
        .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
    let events = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })
        .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;

    let mut has_events = false;
    for e in events {
        let (ts, etype, target, reason) =
            e.map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
        let date = &ts[..std::cmp::min(16, ts.len())];
        let target = target.unwrap_or_default();
        let reason = reason.unwrap_or_default();
        let icon = match etype.as_str() {
            "rule_added" => "✅",
            "example_added" => "📖",
            "skill_generated" => "✨",
            "skill_pending" => "🆕",
            "skill_refined" => "🔧",
            "evolution_judgement" => "🧭",
            "auto_rollback" => "⚠️ ",
            t if t.contains("retired") => "🗑️ ",
            t if t.contains("rolled_back") => "🔙",
            _ => "  ",
        };
        let reason_short = if reason.len() > 50 {
            format!("{}...", &reason[..47])
        } else {
            reason
        };
        println!("  {} {} {} {}", icon, date, etype, reason_short);
        if !target.is_empty() {
            println!("     └─ target: {}", target);
        }
        has_events = true;
    }
    if !has_events {
        println!("  (暂无进化事件)");
    }
    println!();

    Ok(())
}

/// `skilllite evolution reset` — delete all evolved data, return to seed state.
pub fn cmd_reset(force: bool) -> Result<()> {
    if !force {
        println!("⚠️  这将删除所有进化产物（规则、示例、Skill），回到种子状态。");
        println!("   已有进化经验将永久丢失。种子规则不受影响。");
        println!();
        println!("   使用 --force 确认执行。");
        return Ok(());
    }

    let root = paths::chat_root();

    // Re-seed prompts (overwrite evolved rules/examples with seed data)
    skilllite_evolution::seed::ensure_seed_data_force(&root);
    println!("✅ Prompts 已重置为种子状态");

    // Remove evolved skills (project-level, includes _pending)
    let evolved_dir = resolve_skills_root(None).map(|sr| sr.join("_evolved"));
    if let Some(evolved_dir) = evolved_dir.filter(|p| p.exists()) {
        let count = std::fs::read_dir(&evolved_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .count();
        std::fs::remove_dir_all(&evolved_dir)?;
        println!("✅ 已删除 {} 个进化 Skill（含待确认）", count);
    }

    // Clear evolution log entries (but keep decisions for future re-evolution)
    if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&root) {
        conn.execute("DELETE FROM evolution_log", [])
            .map_err(|e| crate::Error::from(anyhow::Error::from(e)))?;
        println!("✅ 已清空进化日志");
    }

    // Remove evolution.log JSONL
    let log_path = root.join("evolution.log");
    if log_path.exists() {
        std::fs::remove_file(&log_path)?;
    }

    // Remove snapshots
    let versions_dir = root.join("prompts").join("_versions");
    if versions_dir.exists() {
        std::fs::remove_dir_all(&versions_dir)?;
    }

    println!();
    println!("🔄 已完成重置。下次对话时将从种子状态重新进化。");

    Ok(())
}

/// `skilllite evolution disable <rule_id>` — disable a specific evolved rule.
pub fn cmd_disable(rule_id: &str) -> Result<()> {
    let root = paths::chat_root();
    let rules_path = root.join("prompts").join("rules.json");

    if !rules_path.exists() {
        bail!("规则文件不存在: {}", rules_path.display());
    }

    let content = std::fs::read_to_string(&rules_path)?;
    let mut rules: Vec<serde_json::Value> = serde_json::from_str(&content)?;

    let pos = rules
        .iter()
        .position(|r| r.get("id").and_then(|v| v.as_str()) == Some(rule_id));

    match pos {
        Some(idx) => {
            let is_mutable = rules[idx]
                .get("mutable")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if !is_mutable {
                bail!("规则 '{}' 是种子规则（不可变），无法禁用", rule_id);
            }
            rules[idx]
                .as_object_mut()
                .context("rule entry is not a JSON object")?
                .insert("disabled".to_string(), serde_json::Value::Bool(true));
            let desc = rules[idx]
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let new_content = serde_json::to_string_pretty(&rules)?;
            std::fs::write(&rules_path, new_content)?;
            println!("✅ 已禁用规则: {}", rule_id);

            if let Some(desc) = desc {
                println!("   描述: {}", desc);
            }
            println!("   (可手动编辑 {} 恢复)", rules_path.display());
        }
        None => {
            bail!("未找到规则: '{}'", rule_id);
        }
    }

    Ok(())
}

/// `skilllite evolution explain <rule_id>` — show rule origin, history, effectiveness.
pub fn cmd_explain(rule_id: &str) -> Result<()> {
    let root = paths::chat_root();

    // Load rule details
    let rules_path = root.join("prompts").join("rules.json");
    if !rules_path.exists() {
        bail!("规则文件不存在: {}", rules_path.display());
    }

    let content = std::fs::read_to_string(&rules_path)?;
    let rules: Vec<serde_json::Value> = serde_json::from_str(&content)?;

    let rule = rules
        .iter()
        .find(|r| r.get("id").and_then(|v| v.as_str()) == Some(rule_id));

    match rule {
        Some(rule) => {
            println!("╭─────────────────────────────────────────────╮");
            println!("│  规则详情: {:33} │", rule_id);
            println!("╰─────────────────────────────────────────────╯");
            println!();

            // rules.json uses "instruction" (seed + evolved), some schemas use description/condition/action
            if let Some(inst) = rule.get("instruction").and_then(|v| v.as_str()) {
                println!("规则: {}", inst);
            }
            if let Some(desc) = rule.get("description").and_then(|v| v.as_str()) {
                println!("描述: {}", desc);
            }
            if let Some(cond) = rule.get("condition").and_then(|v| v.as_str()) {
                println!("条件: {}", cond);
            }
            if let Some(action) = rule.get("action").and_then(|v| v.as_str()) {
                println!("动作: {}", action);
            }
            if let Some(th) = rule
                .get("tool_hint")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && *s != "null")
            {
                println!("建议工具: {}", th);
            }
            if let Some(r) = rule.get("rationale").and_then(|v| v.as_str()) {
                println!("依据: {}", r);
            }

            let mutable = rule
                .get("mutable")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let reusable = rule
                .get("reusable")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let origin = rule
                .get("origin")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let priority = rule.get("priority").and_then(|v| v.as_u64()).unwrap_or(0);
            let disabled = rule
                .get("disabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            println!();
            println!("属性:");
            println!("  来源: {}", origin);
            println!("  优先级: {}", priority);
            println!("  可变: {}", if mutable { "是" } else { "否 (种子规则)" });
            println!("  通用: {}", if reusable { "是 ⬆️" } else { "否" });
            if disabled {
                println!("  状态: ⏸️ 已禁用");
            }

            if let Some(eff) = rule.get("effectiveness").and_then(|v| v.as_f64()) {
                println!("  效果评分: {:.2}", eff);
            }
            if let Some(tc) = rule.get("trigger_count").and_then(|v| v.as_u64()) {
                println!("  触发次数: {}", tc);
            }

            // Evolution history from SQLite
            let conn = skilllite_evolution::feedback::open_evolution_db(&root)?;

            println!();
            println!("进化历史:");
            let history = skilllite_evolution::feedback::query_rule_history(&conn, rule_id)?;
            if history.is_empty() {
                println!("  (无进化历史 — 可能是种子规则)");
            } else {
                for entry in &history {
                    let date = &entry.ts[..std::cmp::min(16, entry.ts.len())];
                    println!(
                        "  {} {} [{}] {}",
                        date, entry.event_type, entry.txn_id, entry.reason
                    );
                }
            }

            // Effectiveness from decisions
            let eff = skilllite_evolution::feedback::compute_effectiveness(&conn, rule_id)?;
            if eff >= 0.0 {
                println!();
                println!("实测效果: {:.0}% (基于关联决策计算)", eff * 100.0);
            }
        }
        None => {
            bail!(
                "未找到规则: '{}'\n提示: 使用 `skilllite evolution status` 查看所有规则",
                rule_id
            );
        }
    }

    Ok(())
}

/// `skilllite evolution confirm <skill_name>` — move pending skill to confirmed (A10).
/// Skills are project-level: moves from _pending to _evolved within workspace/.skills/.
/// Logs skill_confirmed to evolution.log for EvoTown reward (human-approved = effective).
pub fn cmd_confirm(skill_name: &str) -> Result<()> {
    let skills_root = resolve_skills_root(None).ok_or_else(|| {
        crate::Error::validation("无法解析工作区。请在项目目录运行或设置 SKILLLITE_WORKSPACE。")
    })?;
    skilllite_evolution::skill_synth::confirm_pending_skill(&skills_root, skill_name)?;
    let root = paths::chat_root();
    if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&root) {
        let _ = skilllite_evolution::log_evolution_event(
            &conn,
            &root,
            "skill_confirmed",
            skill_name,
            "user confirmed",
            "",
        );
    }
    println!("✅ Skill '{}' 已确认加入", skill_name);
    Ok(())
}

/// `skilllite evolution reject <skill_name>` — remove pending skill without adding (A10).
pub fn cmd_reject(skill_name: &str) -> Result<()> {
    let skills_root = resolve_skills_root(None).ok_or_else(|| {
        crate::Error::validation("无法解析工作区。请在项目目录运行或设置 SKILLLITE_WORKSPACE。")
    })?;
    skilllite_evolution::skill_synth::reject_pending_skill(&skills_root, skill_name)?;
    println!("✅ Skill '{}' 已拒绝", skill_name);
    Ok(())
}

/// `skilllite evolution run` — run evolution once synchronously, output NodeResult with new_skill.
/// Skills are written to workspace/.skills/_evolved/ (project-level).
pub fn cmd_run(json_output: bool) -> Result<()> {
    let root = paths::chat_root();
    let skills_root = resolve_skills_root(None);
    skilllite_core::config::ensure_default_output_dir();

    let config = AgentConfig::from_env();
    if config.api_key.is_empty() {
        bail!("API key required. Set OPENAI_API_KEY env var.");
    }

    let llm = skilllite_agent::llm::LlmClient::new(&config.api_base, &config.api_key)?;
    let adapter = skilllite_agent::evolution::EvolutionLlmAdapter { llm: &llm };

    let rt = tokio::runtime::Runtime::new().context("tokio runtime init failed")?;
    let run_result = rt.block_on(skilllite_evolution::run_evolution(
        &root,
        skills_root.as_deref(),
        &adapter,
        &config.api_base,
        &config.api_key,
        &config.model,
        true, // force: manual trigger bypasses decision thresholds
    ))?;

    let task_id = uuid::Uuid::new_v4().to_string();
    let response = match run_result {
        skilllite_evolution::EvolutionRunResult::Completed(Some(txn_id)) => {
            let conn = skilllite_evolution::feedback::open_evolution_db(&root)?;
            let changes = skilllite_evolution::query_changes_by_txn(&conn, &txn_id);

            if changes.iter().any(|(t, _)| t == "memory_knowledge_added") {
                let _ = skilllite_agent::extensions::index_evolution_knowledge(&root, "default");
            }

            let new_skill = changes
                .iter()
                .find(|(t, _)| t == "skill_pending" || t == "skill_refined")
                .and_then(|(_, skill_name)| {
                    skills_root
                        .as_ref()
                        .and_then(|sr| build_new_skill(sr, skill_name, &txn_id))
                });

            let summary: Vec<String> = skilllite_evolution::format_evolution_changes(&changes);
            let response_text = if summary.is_empty() {
                format!("Evolution completed (txn={})", txn_id)
            } else {
                summary.join("\n")
            };

            NodeResult {
                task_id: task_id.clone(),
                response: response_text,
                task_completed: true,
                tool_calls: 0,
                new_skill,
            }
        }
        skilllite_evolution::EvolutionRunResult::SkippedBusy => NodeResult {
            task_id: task_id.clone(),
            response: "Evolution skipped: another run in progress".to_string(),
            task_completed: true,
            tool_calls: 0,
            new_skill: None,
        },
        skilllite_evolution::EvolutionRunResult::NoScope
        | skilllite_evolution::EvolutionRunResult::Completed(None) => {
            // Diagnostic: help user understand why
            let mut hint = String::from("Evolution: nothing to evolve");
            if let Ok(conn) = skilllite_evolution::feedback::open_evolution_db(&root) {
                if let Ok((total, with_desc)) =
                    skilllite_evolution::feedback::count_decisions_with_task_desc(&conn)
                {
                    if total > 0 && with_desc == 0 {
                        hint.push_str("\n\n提示: 进化需要 task_description。当前未进化决策均无 task_description。");
                        hint.push_str("\n请使用最新构建: cargo build && ./target/debug/skilllite run --goal \"...\"");
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
            NodeResult {
                task_id: task_id.clone(),
                response: hint,
                task_completed: true,
                tool_calls: 0,
                new_skill: None,
            }
        }
    };

    if json_output {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("{}", response.response);
        if let Some(ref ns) = response.new_skill {
            println!();
            println!("🆕 NewSkill: {} (path: {})", ns.name, ns.path);
            println!("   → 确认: skilllite evolution confirm {}", ns.name);
        }
    }

    Ok(())
}

fn build_new_skill(skills_root: &Path, skill_name: &str, txn_id: &str) -> Option<NewSkill> {
    let pending_path = skills_root
        .join("_evolved")
        .join("_pending")
        .join(skill_name);
    let evolved_path = skills_root.join("_evolved").join(skill_name);

    let (path, skill_dir) = if pending_path.exists() {
        (pending_path.clone(), pending_path)
    } else if evolved_path.exists() {
        (evolved_path.clone(), evolved_path)
    } else {
        return None;
    };

    let description = skilllite_core::skill::metadata::parse_skill_metadata(&skill_dir)
        .ok()
        .and_then(|m| m.description)
        .unwrap_or_else(|| skill_name.to_string());

    Some(NewSkill {
        name: skill_name.to_string(),
        description,
        path: path.to_string_lossy().to_string(),
        txn_id: txn_id.to_string(),
    })
}

/// 判断 source 是否为远程（非本地路径），用于区分「下载的技能」与本地/进化技能
fn is_remote_source(source: &str) -> bool {
    let s = source.trim();
    if s.is_empty() {
        return false;
    }
    let path = Path::new(s);
    !path.is_absolute() && !s.starts_with("./") && !s.starts_with("../") && s != "." && s != ".."
}

/// 判断 source 是否可被实际拉取（URL / clawhub / user/repo），而非仅作标识（如 evotown-arena）
fn is_fetchable_source(source: &str) -> bool {
    let s = source.trim();
    if s.is_empty() {
        return false;
    }
    if let Some(stripped) = s.strip_prefix("clawhub:") {
        return !stripped.trim().is_empty();
    }
    if Path::new(s).is_absolute()
        || s.starts_with("./")
        || s.starts_with("../")
        || s == "."
        || s == ".."
    {
        return true;
    }
    if s.contains("://") || s.contains('@') {
        return true;
    }
    if s.contains('/') && !s.starts_with('.') {
        return true;
    }
    false
}

/// `skilllite evolution repair-skills [SKILL_NAME...]` — 验证技能并修复失败的。
/// 不传技能名时验证并修复所有失败技能；传一个或多个技能名时仅验证并修复这些技能，缩短执行时间。
/// `from_source`: 对下载的技能失败时自动从源头更新，不交互询问（桌面/CI 等非 TTY 时传 true）。
pub fn cmd_repair_skills(skills_filter: Option<Vec<String>>, from_source: bool) -> Result<()> {
    let skills_root = resolve_skills_root(None).ok_or_else(|| {
        crate::Error::validation("无法解析工作区。请设置 SKILLLITE_WORKSPACE 或在项目目录运行。")
    })?;

    let config = AgentConfig::from_env();
    if config.api_key.is_empty() {
        bail!("API key required. Set OPENAI_API_KEY or SKILLLITE_API_KEY env var.");
    }

    let llm = skilllite_agent::llm::LlmClient::new(&config.api_base, &config.api_key)?;
    let adapter = skilllite_agent::evolution::EvolutionLlmAdapter { llm: &llm };

    let rt = tokio::runtime::Runtime::new().context("tokio runtime init failed")?;

    let filter_ref = skills_filter.as_deref();
    if let Some(names) = filter_ref {
        if !names.is_empty() {
            eprintln!("🔧 仅验证/修复指定技能: {}", names.join(", "));
        }
    }

    let validated = rt.block_on(skilllite_evolution::skill_synth::validate_skills(
        &skills_root,
        &adapter,
        &config.model,
        filter_ref,
    ))?;

    let failed: Vec<_> = validated.iter().filter(|v| !v.passed).collect();
    if failed.is_empty() {
        println!("🔧 所有技能验证通过，无需修复。");
        return Ok(());
    }

    let manifest = manifest::load_manifest(&skills_root).unwrap_or_default();
    let mut results: Vec<(String, bool)> = validated
        .iter()
        .filter(|v| v.passed)
        .map(|v| (v.skill_name.clone(), true))
        .collect();

    println!(
        "\n🔧 修复 {} 个失败的技能（进化的→大模型修复，下载的→可选从源头更新）...",
        failed.len()
    );

    for (idx, v) in validated.iter().filter(|v| !v.passed).enumerate() {
        let (ep, ti) = match (&v.entry_point, &v.test_input) {
            (Some(ep), Some(ti)) => (ep.as_str(), ti.as_str()),
            _ => {
                eprintln!("  ⏭️ {} (推理失败，跳过)", v.skill_name);
                results.push((v.skill_name.clone(), false));
                continue;
            }
        };

        let is_evolved = v
            .skill_dir
            .as_os_str()
            .to_string_lossy()
            .contains("_evolved");
        let manifest_key = v
            .skill_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let entry = if manifest_key.is_empty() {
            None
        } else {
            manifest.skills.get(manifest_key).cloned()
        };

        let ok = if is_evolved {
            eprintln!(
                "🔧 [{}/{}] {}（进化技能，大模型修复）...",
                idx + 1,
                failed.len(),
                v.skill_name
            );
            let on_msg = |msg: &str| eprintln!("  💬 {}", msg);
            let (ok, reason) = rt
                .block_on(skilllite_evolution::skill_synth::repair_one_skill(
                    &adapter,
                    &config.model,
                    &v.skill_dir,
                    &v.skill_name,
                    ep,
                    ti,
                    Some(&on_msg),
                ))
                .unwrap_or_else(|e| (false, format!("{}", e)));
            if !ok && !reason.is_empty() {
                eprintln!("  ❌ {}", reason);
            }
            ok
        } else if let Some(ref e) = entry {
            if is_remote_source(&e.source) && is_fetchable_source(&e.source) {
                eprintln!(
                    "🔧 [{}/{}] {}（来自 {}）",
                    idx + 1,
                    failed.len(),
                    v.skill_name,
                    e.source
                );
                let yes = if from_source {
                    true
                } else {
                    print!("  是否从源头更新？(y/n) [n]: ");
                    let _ = io::stdout().flush();
                    let mut line = String::new();
                    if io::stdin().read_line(&mut line).is_err() {
                        eprintln!("  ⏭️ 跳过（非交互环境，可加 --from-source 自动从源头更新）");
                        false
                    } else {
                        line.trim().eq_ignore_ascii_case("y")
                            || line.trim().eq_ignore_ascii_case("yes")
                    }
                };
                if yes {
                    match crate::skill::update_skill_from_source(
                        &skills_root,
                        &v.skill_name,
                        &e.source,
                    ) {
                        Ok(()) => {
                            eprintln!("  ✅ 已从源头更新");
                            true
                        }
                        Err(err) => {
                            eprintln!("  ❌ 更新失败: {}", err);
                            eprintln!("  💡 可改用大模型修复：重新执行 repair-skills 并选 n 后会自动用大模型修");
                            false
                        }
                    }
                } else {
                    eprintln!("  ⏭️ 已跳过");
                    false
                }
            } else if is_remote_source(&e.source) && !is_fetchable_source(&e.source) {
                eprintln!(
                    "🔧 [{}/{}] {}（来源为标识「{}」，无法拉取，大模型修复）...",
                    idx + 1,
                    failed.len(),
                    v.skill_name,
                    e.source
                );
                let on_msg = |msg: &str| eprintln!("  💬 {}", msg);
                let (ok, reason) = rt
                    .block_on(skilllite_evolution::skill_synth::repair_one_skill(
                        &adapter,
                        &config.model,
                        &v.skill_dir,
                        &v.skill_name,
                        ep,
                        ti,
                        Some(&on_msg),
                    ))
                    .unwrap_or_else(|e| (false, format!("{}", e)));
                if !ok && !reason.is_empty() {
                    eprintln!("  ❌ {}", reason);
                }
                ok
            } else {
                eprintln!(
                    "🔧 [{}/{}] {}（大模型修复）...",
                    idx + 1,
                    failed.len(),
                    v.skill_name
                );
                let on_msg = |msg: &str| eprintln!("  💬 {}", msg);
                let (ok, reason) = rt
                    .block_on(skilllite_evolution::skill_synth::repair_one_skill(
                        &adapter,
                        &config.model,
                        &v.skill_dir,
                        &v.skill_name,
                        ep,
                        ti,
                        Some(&on_msg),
                    ))
                    .unwrap_or_else(|e| (false, format!("{}", e)));
                if !ok && !reason.is_empty() {
                    eprintln!("  ❌ {}", reason);
                }
                ok
            }
        } else {
            eprintln!(
                "🔧 [{}/{}] {}（大模型修复）...",
                idx + 1,
                failed.len(),
                v.skill_name
            );
            let on_msg = |msg: &str| eprintln!("  💬 {}", msg);
            let (ok, reason) = rt
                .block_on(skilllite_evolution::skill_synth::repair_one_skill(
                    &adapter,
                    &config.model,
                    &v.skill_dir,
                    &v.skill_name,
                    ep,
                    ti,
                    Some(&on_msg),
                ))
                .unwrap_or_else(|e| (false, format!("{}", e)));
            if !ok && !reason.is_empty() {
                eprintln!("  ❌ {}", reason);
            }
            ok
        };

        results.push((v.skill_name.clone(), ok));
    }

    let ok_count = results.iter().filter(|(_, ok)| *ok).count();
    let fail_count = results.len() - ok_count;
    println!(
        "\n🔧 技能修复完成: 共 {} 个技能, {} 成功, {} 失败",
        results.len(),
        ok_count,
        fail_count
    );
    for (name, ok) in &results {
        println!(
            "  {} {} {}",
            if *ok { "✅" } else { "❌" },
            name,
            if *ok {
                "(通过/已更新)"
            } else {
                "(未通过)"
            }
        );
    }

    Ok(())
}
