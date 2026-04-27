//! Analyze `audit_*.jsonl` for supply-chain observability (P1): counts, failure rates, edit paths; optional alerts.

use anyhow::Context;
use chrono::{DateTime, Duration, Utc};

use crate::error::bail;
use crate::Result;
use serde::Serialize;
use serde_json::Value;
use skilllite_core::config::env_keys::observability;
use skilllite_core::config::loader::env_optional;
use skilllite_core::config::ObservabilityConfig;
use skilllite_core::paths::data_root;
use std::collections::{HashMap, HashSet};
use std::fs::{read_dir, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const DEFAULT_MAX_INVOCATIONS: u64 = 200;
const DEFAULT_MIN_INVOCATIONS_FOR_FAILURE: u64 = 5;
const DEFAULT_FAILURE_RATIO: f64 = 0.5;
const DEFAULT_EDIT_UNIQUE_PATHS: u64 = 80;

#[derive(Debug, Default, Clone)]
struct SkillInvoStats {
    total: u64,
    failures: u64,
}

#[derive(Debug, Default)]
struct ReportAccum {
    /// skill_id -> stats from `skill_invocation` events
    invocations: HashMap<String, SkillInvoStats>,
    /// distinct paths from edit_* events
    edit_paths: HashSet<String>,
}

#[derive(Debug, Serialize)]
struct ReportJson {
    window_hours: u64,
    /// Inclusive lower bound for event `ts` (same as historical field name `cutoff`).
    cutoff: String,
    /// Exclusive upper bound is "now" at report generation time.
    window_end: String,
    audit_files_read: Vec<String>,
    skill_invocations: HashMap<String, SkillInvoStatsJson>,
    edit_distinct_paths: u64,
    top_edit_paths: Vec<(String, u64)>,
    alerts: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SkillInvoStatsJson {
    total: u64,
    failures: u64,
    failure_ratio: f64,
}

fn env_u64(key: &str, default: u64) -> u64 {
    env_optional(key, &[])
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(default)
}

fn env_f64(key: &str, default: f64) -> f64 {
    env_optional(key, &[])
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(default)
}

fn parse_line_ts(v: &Value) -> Option<DateTime<Utc>> {
    v.get("ts")
        .and_then(|t| t.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn audit_jsonl_paths(audit_dir: &Path) -> Result<Vec<PathBuf>> {
    let rd =
        read_dir(audit_dir).with_context(|| format!("read audit dir {}", audit_dir.display()))?;
    let mut out: Vec<PathBuf> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("audit_") && n.ends_with(".jsonl"))
        })
        .collect();
    out.sort();
    Ok(out)
}

fn resolve_audit_dir(cli_dir: Option<&str>) -> PathBuf {
    if let Some(d) = cli_dir.filter(|s| !s.is_empty()) {
        return PathBuf::from(d);
    }
    if let Some(cfg) = ObservabilityConfig::from_env().audit_log.as_ref() {
        let p = Path::new(cfg);
        if cfg.ends_with(".jsonl") {
            if let Some(parent) = p.parent() {
                return parent.to_path_buf();
            }
        } else {
            return p.to_path_buf();
        }
    }
    data_root().join("audit")
}

fn process_file(path: &Path, cutoff: DateTime<Utc>, acc: &mut ReportAccum) -> Result<u64> {
    let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut lines_used: u64 = 0;
    for line in BufReader::new(f).lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(ts) = parse_line_ts(&v) else {
            continue;
        };
        if ts < cutoff {
            continue;
        }
        lines_used += 1;
        let event = v.get("event").and_then(|e| e.as_str()).unwrap_or("");
        match event {
            "skill_invocation" => {
                let skill_id = v
                    .get("skill_id")
                    .and_then(|s| s.as_str())
                    .unwrap_or("")
                    .to_string();
                if skill_id.is_empty() {
                    continue;
                }
                let success = v.get("success").and_then(|b| b.as_bool()).unwrap_or(true);
                let e = acc.invocations.entry(skill_id).or_default();
                e.total += 1;
                if !success {
                    e.failures += 1;
                }
            }
            "edit_applied" | "edit_previewed" | "edit_inserted" | "edit_failed" => {
                if let Some(p) = v.get("path").and_then(|x| x.as_str()) {
                    if !p.is_empty() {
                        acc.edit_paths.insert(p.to_string());
                    }
                }
            }
            _ => {}
        }
    }
    Ok(lines_used)
}

fn count_edit_paths_by_file(path: &Path, cutoff: DateTime<Utc>) -> Result<HashMap<String, u64>> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    let f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    for line in BufReader::new(f).lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let v: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(ts) = parse_line_ts(&v) else {
            continue;
        };
        if ts < cutoff {
            continue;
        }
        let event = v.get("event").and_then(|e| e.as_str()).unwrap_or("");
        if !matches!(
            event,
            "edit_applied" | "edit_previewed" | "edit_inserted" | "edit_failed"
        ) {
            continue;
        }
        if let Some(p) = v.get("path").and_then(|x| x.as_str()) {
            if !p.is_empty() {
                *counts.entry(p.to_string()).or_insert(0) += 1;
            }
        }
    }
    Ok(counts)
}

fn eval_alerts(
    acc: &ReportAccum,
    top_edit_paths: &[(String, u64)],
    max_inv: u64,
    min_inv_fail: u64,
    fail_ratio: f64,
    edit_paths_alert: u64,
) -> Vec<String> {
    let mut alerts = Vec::new();
    for (name, st) in &acc.invocations {
        if st.total >= max_inv {
            alerts.push(format!(
                "skill_invocation: skill_id {:?} has {} invocations (threshold {})",
                name, st.total, max_inv
            ));
        }
        if st.total >= min_inv_fail {
            let ratio = st.failures as f64 / st.total as f64;
            if ratio >= fail_ratio {
                alerts.push(format!(
                    "skill_invocation: skill_id {:?} failure ratio {:.1}% over {} calls (threshold {:.0}% over {}+ calls)",
                    name,
                    ratio * 100.0,
                    st.total,
                    fail_ratio * 100.0,
                    min_inv_fail
                ));
            }
        }
    }
    let distinct = acc.edit_paths.len() as u64;
    if distinct >= edit_paths_alert {
        alerts.push(format!(
            "edit_*: {} distinct paths touched (threshold {})",
            distinct, edit_paths_alert
        ));
    }
    // Optional: many touches on a single path
    if let Some((p, n)) = top_edit_paths.first() {
        if *n >= max_inv {
            alerts.push(format!(
                "edit_*: path {:?} has {} events (threshold {})",
                p, n, max_inv
            ));
        }
    }
    alerts
}

#[cfg(feature = "audit")]
fn post_webhook(url: &str, payload: &Value) -> Result<()> {
    let body = serde_json::to_string(payload)?;
    ureq::post(url)
        .set("Content-Type", "application/json; charset=utf-8")
        .send_string(&body)
        .map_err(|e| crate::Error::validation(format!("webhook POST failed: {}", e)))?;
    Ok(())
}

#[cfg(not(feature = "audit"))]
fn post_webhook(url: &str, payload: &Value) -> Result<()> {
    let _ = (url, payload);
    bail!("webhook requires skilllite-commands built with feature \"audit\" (ureq)");
}

/// Summarize audit JSONL under `audit_dir` for the last `hours` hours.
pub fn cmd_audit_report(
    audit_dir: Option<&str>,
    hours: u64,
    json_output: bool,
    alert: bool,
    webhook: Option<&str>,
) -> Result<()> {
    let dir = resolve_audit_dir(audit_dir);
    if !dir.is_dir() {
        bail!(
            "Audit directory not found: {}. Set --dir or SKILLLITE_AUDIT_LOG.",
            dir.display()
        );
    }

    let paths = audit_jsonl_paths(&dir)?;
    if paths.is_empty() {
        bail!("No audit_*.jsonl files under {}.", dir.display());
    }

    let end = Utc::now();
    let cutoff = end - Duration::hours(hours as i64);

    let max_inv = env_u64(
        observability::SKILLLITE_AUDIT_ALERT_MAX_INVOCATIONS_PER_SKILL,
        DEFAULT_MAX_INVOCATIONS,
    );
    let min_inv_fail = env_u64(
        observability::SKILLLITE_AUDIT_ALERT_MIN_INVOCATIONS_FOR_FAILURE,
        DEFAULT_MIN_INVOCATIONS_FOR_FAILURE,
    );
    let fail_ratio = env_f64(
        observability::SKILLLITE_AUDIT_ALERT_FAILURE_RATIO,
        DEFAULT_FAILURE_RATIO,
    );
    let edit_paths_threshold = env_u64(
        observability::SKILLLITE_AUDIT_ALERT_EDIT_UNIQUE_PATHS,
        DEFAULT_EDIT_UNIQUE_PATHS,
    );

    let mut acc = ReportAccum::default();
    let mut files_read: Vec<String> = Vec::new();
    for p in &paths {
        let n = process_file(p, cutoff, &mut acc)?;
        if n > 0 {
            files_read.push(p.to_string_lossy().into_owned());
        }
    }

    let mut path_counts: HashMap<String, u64> = HashMap::new();
    for p in &paths {
        let part = count_edit_paths_by_file(p, cutoff)?;
        for (k, v) in part {
            *path_counts.entry(k).or_insert(0) += v;
        }
    }
    let mut top_edit: Vec<(String, u64)> = path_counts.into_iter().collect();
    top_edit.sort_by_key(|item| std::cmp::Reverse(item.1));
    top_edit.truncate(20);

    let alerts = eval_alerts(
        &acc,
        &top_edit,
        max_inv,
        min_inv_fail,
        fail_ratio,
        edit_paths_threshold,
    );

    let skill_json: HashMap<String, SkillInvoStatsJson> = acc
        .invocations
        .iter()
        .map(|(k, st)| {
            let ratio = if st.total > 0 {
                st.failures as f64 / st.total as f64
            } else {
                0.0
            };
            (
                k.clone(),
                SkillInvoStatsJson {
                    total: st.total,
                    failures: st.failures,
                    failure_ratio: ratio,
                },
            )
        })
        .collect();

    if json_output {
        let report = ReportJson {
            window_hours: hours,
            cutoff: cutoff.to_rfc3339(),
            window_end: end.to_rfc3339(),
            audit_files_read: files_read,
            skill_invocations: skill_json,
            edit_distinct_paths: acc.edit_paths.len() as u64,
            top_edit_paths: top_edit.clone(),
            alerts: alerts.clone(),
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Audit report — last {} hour(s) (UTC)", hours);
        println!(
            "Time window: {} → {}  (events with ts in this range are counted)",
            cutoff.to_rfc3339(),
            end.to_rfc3339()
        );
        println!("Directory: {}", dir.display());
        println!("JSONL files with ≥1 event in window: {}", files_read.len());
        println!("\nPer-skill invocations (skill_invocation):");
        let mut names: Vec<_> = acc.invocations.keys().cloned().collect();
        names.sort();
        for name in names {
            let st = acc.invocations.get(&name).unwrap();
            let ratio = if st.total > 0 {
                st.failures as f64 / st.total as f64
            } else {
                0.0
            };
            println!(
                "  {}  total={} failures={} fail_rate={:.1}%",
                name,
                st.total,
                st.failures,
                ratio * 100.0
            );
        }
        println!(
            "\nEdit events: {} distinct paths (top {} shown)",
            acc.edit_paths.len(),
            top_edit.len().min(20)
        );
        for (p, c) in &top_edit {
            println!("  {}  events={}", p, c);
        }
        if !alerts.is_empty() {
            println!("\nAlerts (rule hits):");
            for a in &alerts {
                println!("  - {}", a);
            }
        }
    }

    if alert && !alerts.is_empty() {
        for a in &alerts {
            tracing::warn!(target: "skilllite::audit", "{}", a);
            eprintln!("[skilllite audit alert] {}", a);
        }
        let wh = webhook
            .map(|s| s.to_string())
            .or_else(|| env_optional(observability::SKILLLITE_AUDIT_ALERT_WEBHOOK, &[]));
        if let Some(url) = wh.filter(|s| !s.is_empty()) {
            let payload = serde_json::json!({
                "ts": Utc::now().to_rfc3339(),
                "alerts": alerts,
                "window_hours": hours,
                "audit_dir": dir.to_string_lossy(),
            });
            post_webhook(&url, &payload)?;
        }
    }

    Ok(())
}
