//! EVO-5: Evolution management CLI commands.
//!
//! Provides `skilllite evolution {status,reset,disable,explain}` subcommands
//! for inspecting, controlling, and debugging the self-evolution engine.

use anyhow::{Context, Result};
use std::path::PathBuf;

fn chat_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".skilllite")
        .join("chat")
}

/// `skilllite evolution status` — show evolution statistics, effectiveness, trends.
pub fn cmd_status() -> Result<()> {
    let root = chat_root();
    let conn = skilllite_agent::evolution::feedback::open_evolution_db(&root)?;
    let mode = skilllite_agent::evolution::EvolutionMode::from_env();

    // Header
    println!("╭─────────────────────────────────────────────╮");
    println!("│       SkillLite 自进化引擎状态               │");
    println!("╰─────────────────────────────────────────────╯");
    println!();

    // Mode
    let mode_str = match &mode {
        skilllite_agent::evolution::EvolutionMode::All => "全部启用 ✅",
        skilllite_agent::evolution::EvolutionMode::PromptsOnly => "仅 Prompts",
        skilllite_agent::evolution::EvolutionMode::MemoryOnly => "仅 Memory",
        skilllite_agent::evolution::EvolutionMode::SkillsOnly => "仅 Skills",
        skilllite_agent::evolution::EvolutionMode::Disabled => "已禁用 ⏸️  (已有进化产物冻结生效中)",
    };
    println!("进化模式: {}", mode_str);
    println!();

    // Evolution counts
    let total_evolutions: i64 = conn
        .query_row("SELECT COUNT(*) FROM evolution_log", [], |r| r.get(0))
        .unwrap_or(0);
    let today_evolutions: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evolution_log WHERE date(ts) = date('now')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let total_decisions: i64 = conn
        .query_row("SELECT COUNT(*) FROM decisions", [], |r| r.get(0))
        .unwrap_or(0);
    let rollback_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM evolution_log WHERE type = 'auto_rollback'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    println!("📊 概览");
    println!("  总进化次数: {}", total_evolutions);
    println!("  今日进化次数: {}", today_evolutions);
    println!("  总决策记录: {}", total_decisions);
    println!("  自动回滚次数: {}", rollback_count);
    println!();

    // A14: 进化队列与待确认列表
    let unprocessed: i64 = conn
        .query_row("SELECT COUNT(*) FROM decisions WHERE evolved = 0", [], |r| r.get(0))
        .unwrap_or(0);
    let pending = skilllite_agent::evolution::skill_synth::list_pending_skills(&root);

    println!("📥 进化队列与待确认");
    println!("  进化队列: {} 条决策待处理 (空闲 5 分钟或周期性触发时进化)", unprocessed);
    if !pending.is_empty() {
        println!("  待确认 Skill: {}", pending.join(", "));
        println!("    → 确认: skilllite evolution confirm <name>");
        println!("    → 拒绝: skilllite evolution reject <name>");
    } else {
        println!("  待确认 Skill: (无)");
    }
    println!();

    // Evolved rules summary
    let rules_path = root.join("prompts").join("rules.json");
    if rules_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&rules_path) {
            if let Ok(rules) = serde_json::from_str::<Vec<serde_json::Value>>(&content) {
                let total = rules.len();
                let mutable = rules.iter().filter(|r| r.get("mutable").and_then(|v| v.as_bool()).unwrap_or(true)).count();
                let reusable = rules.iter().filter(|r| r.get("reusable").and_then(|v| v.as_bool()).unwrap_or(false)).count();
                let immutable = total - mutable;
                println!("📋 规则");
                println!("  总规则数: {} (种子: {}, 可变: {}, 通用: {})", total, immutable, mutable, reusable);
            }
        }
    }

    // Evolved skills count
    let evolved_dir = root.join("skills").join("_evolved");
    if evolved_dir.exists() {
        let active = std::fs::read_dir(&evolved_dir)
            .ok()
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                !name.starts_with('_') && e.file_type().map(|t| t.is_dir()).unwrap_or(false)
            })
            .filter(|e| {
                let meta = e.path().join(".meta.json");
                if meta.exists() {
                    if let Ok(content) = std::fs::read_to_string(&meta) {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                            return v.get("archived").and_then(|v| v.as_bool()).unwrap_or(false) == false;
                        }
                    }
                }
                true
            })
            .count();
        println!("  进化 Skill 数: {} (活跃)", active);
    }
    println!();

    // Recent metrics trend
    println!("📈 系统指标趋势 (最近 7 天)");
    println!("  {:10} {:>8} {:>8} {:>8} {:>6}", "日期", "成功率", "Replan", "纠正率", "EGL");
    println!("  {:10} {:>8} {:>8} {:>8} {:>6}", "──────────", "────────", "────────", "────────", "──────");

    let mut stmt = conn.prepare(
        "SELECT date, first_success_rate, avg_replans, user_correction_rate, egl
         FROM evolution_metrics
         WHERE date > date('now', '-7 days') ORDER BY date DESC",
    )?;
    let metrics = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, f64>(3)?,
            row.get::<_, f64>(4)?,
        ))
    })?;

    let mut has_metrics = false;
    for m in metrics {
        let (date, fsr, avg_r, ucr, egl) = m?;
        println!(
            "  {:10} {:>7.0}% {:>8.1} {:>7.0}% {:>6.1}",
            date,
            fsr * 100.0,
            avg_r,
            ucr * 100.0,
            egl,
        );
        has_metrics = true;
    }
    if !has_metrics {
        println!("  (暂无数据 — 需要更多使用后才会出现)");
    }
    println!();

    // Recent evolution events
    println!("📜 最近进化事件");
    let mut stmt = conn.prepare(
        "SELECT ts, type, target_id, reason FROM evolution_log
         ORDER BY ts DESC LIMIT 10",
    )?;
    let events = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    let mut has_events = false;
    for e in events {
        let (ts, etype, target, reason) = e?;
        let date = &ts[..std::cmp::min(16, ts.len())];
        let target = target.unwrap_or_default();
        let reason = reason.unwrap_or_default();
        let icon = match etype.as_str() {
            "rule_added" => "✅",
            "example_added" => "📖",
            "skill_generated" => "✨",
            "skill_pending" => "🆕",
            "skill_refined" => "🔧",
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

    // Time trends
    println!("🕐 活跃时段分布 (最近 30 天)");
    match skilllite_agent::evolution::feedback::query_peak_hours(&conn) {
        Ok(peaks) if !peaks.is_empty() => {
            let peak_strs: Vec<String> = peaks
                .iter()
                .map(|(h, c)| format!("{:02}:00 ({}次)", h, c))
                .collect();
            println!("  高峰时段: {}", peak_strs.join(", "));
        }
        _ => println!("  (暂无数据)"),
    }

    match skilllite_agent::evolution::feedback::query_weekday_activity(&conn) {
        Ok(days) if !days.is_empty() => {
            print!("  星期分布: ");
            let day_strs: Vec<String> = days
                .iter()
                .map(|d| format!("{}: {}次", d.weekday_name, d.count))
                .collect();
            println!("{}", day_strs.join(" | "));
        }
        _ => {}
    }

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

    let root = chat_root();

    // Re-seed prompts (overwrite evolved rules/examples with seed data)
    skilllite_agent::evolution::seed::ensure_seed_data_force(&root);
    println!("✅ Prompts 已重置为种子状态");

    // Remove evolved skills (includes _pending)
    let evolved_dir = root.join("skills").join("_evolved");
    if evolved_dir.exists() {
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
    if let Ok(conn) = skilllite_agent::evolution::feedback::open_evolution_db(&root) {
        conn.execute("DELETE FROM evolution_log", [])?;
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
    let root = chat_root();
    let rules_path = root.join("prompts").join("rules.json");

    if !rules_path.exists() {
        anyhow::bail!("规则文件不存在: {}", rules_path.display());
    }

    let content = std::fs::read_to_string(&rules_path)?;
    let mut rules: Vec<serde_json::Value> = serde_json::from_str(&content)?;

    let pos = rules.iter().position(|r| {
        r.get("id").and_then(|v| v.as_str()) == Some(rule_id)
    });

    match pos {
        Some(idx) => {
            let is_mutable = rules[idx].get("mutable").and_then(|v| v.as_bool()).unwrap_or(true);
            if !is_mutable {
                anyhow::bail!("规则 '{}' 是种子规则（不可变），无法禁用", rule_id);
            }
            rules[idx]
                .as_object_mut()
                .context("rule entry is not a JSON object")?
                .insert("disabled".to_string(), serde_json::Value::Bool(true));
            let desc = rules[idx].get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
            let new_content = serde_json::to_string_pretty(&rules)?;
            std::fs::write(&rules_path, new_content)?;
            println!("✅ 已禁用规则: {}", rule_id);

            if let Some(desc) = desc {
                println!("   描述: {}", desc);
            }
            println!("   (可手动编辑 {} 恢复)", rules_path.display());
        }
        None => {
            anyhow::bail!("未找到规则: '{}'", rule_id);
        }
    }

    Ok(())
}

/// `skilllite evolution explain <rule_id>` — show rule origin, history, effectiveness.
pub fn cmd_explain(rule_id: &str) -> Result<()> {
    let root = chat_root();

    // Load rule details
    let rules_path = root.join("prompts").join("rules.json");
    if !rules_path.exists() {
        anyhow::bail!("规则文件不存在: {}", rules_path.display());
    }

    let content = std::fs::read_to_string(&rules_path)?;
    let rules: Vec<serde_json::Value> = serde_json::from_str(&content)?;

    let rule = rules.iter().find(|r| {
        r.get("id").and_then(|v| v.as_str()) == Some(rule_id)
    });

    match rule {
        Some(rule) => {
            println!("╭─────────────────────────────────────────────╮");
            println!("│  规则详情: {:33} │", rule_id);
            println!("╰─────────────────────────────────────────────╯");
            println!();

            if let Some(desc) = rule.get("description").and_then(|v| v.as_str()) {
                println!("描述: {}", desc);
            }
            if let Some(cond) = rule.get("condition").and_then(|v| v.as_str()) {
                println!("条件: {}", cond);
            }
            if let Some(action) = rule.get("action").and_then(|v| v.as_str()) {
                println!("动作: {}", action);
            }

            let mutable = rule.get("mutable").and_then(|v| v.as_bool()).unwrap_or(true);
            let reusable = rule.get("reusable").and_then(|v| v.as_bool()).unwrap_or(false);
            let origin = rule.get("origin").and_then(|v| v.as_str()).unwrap_or("unknown");
            let priority = rule.get("priority").and_then(|v| v.as_u64()).unwrap_or(0);
            let disabled = rule.get("disabled").and_then(|v| v.as_bool()).unwrap_or(false);

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
            let conn = skilllite_agent::evolution::feedback::open_evolution_db(&root)?;

            println!();
            println!("进化历史:");
            let history = skilllite_agent::evolution::feedback::query_rule_history(&conn, rule_id)?;
            if history.is_empty() {
                println!("  (无进化历史 — 可能是种子规则)");
            } else {
                for entry in &history {
                    let date = &entry.ts[..std::cmp::min(16, entry.ts.len())];
                    println!("  {} {} [{}] {}", date, entry.event_type, entry.txn_id, entry.reason);
                }
            }

            // Effectiveness from decisions
            let eff = skilllite_agent::evolution::feedback::compute_effectiveness(&conn, rule_id)?;
            if eff >= 0.0 {
                println!();
                println!("实测效果: {:.0}% (基于关联决策计算)", eff * 100.0);
            }
        }
        None => {
            anyhow::bail!("未找到规则: '{}'\n提示: 使用 `skilllite evolution status` 查看所有规则", rule_id);
        }
    }

    Ok(())
}

/// `skilllite evolution confirm <skill_name>` — move pending skill to confirmed (A10).
pub fn cmd_confirm(skill_name: &str) -> Result<()> {
    let root = chat_root();
    skilllite_agent::evolution::skill_synth::confirm_pending_skill(&root, skill_name)?;
    println!("✅ Skill '{}' 已确认加入", skill_name);
    Ok(())
}

/// `skilllite evolution reject <skill_name>` — remove pending skill without adding (A10).
pub fn cmd_reject(skill_name: &str) -> Result<()> {
    let root = chat_root();
    skilllite_agent::evolution::skill_synth::reject_pending_skill(&root, skill_name)?;
    println!("✅ Skill '{}' 已拒绝", skill_name);
    Ok(())
}
