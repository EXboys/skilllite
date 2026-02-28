//! Admission scanning: static analysis + LLM-based risk assessment.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use skilllite_core::skill::metadata;
use skilllite_sandbox::security::ScriptScanner;
use skilllite_sandbox::security::types::SecuritySeverity;

#[cfg(feature = "agent")]
use skilllite_agent::llm::LlmClient;
#[cfg(feature = "agent")]
use skilllite_agent::types::{AgentConfig, ChatMessage};

fn collect_script_files(skill_path: &Path, meta: &metadata::SkillMetadata) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if !meta.entry_point.is_empty() {
        let ep = skill_path.join(&meta.entry_point);
        if ep.exists() {
            if let Ok(canonical) = ep.canonicalize() {
                seen.insert(canonical);
            }
            files.push(ep);
        }
    }

    let scripts_dir = skill_path.join("scripts");
    if scripts_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&scripts_dir) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let dominated = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| matches!(ext, "py" | "js" | "ts" | "sh"))
                    .unwrap_or(false);
                if !dominated {
                    continue;
                }
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with("test_")
                    || name.ends_with("_test.py")
                    || name == "__init__.py"
                    || name.starts_with('.')
                {
                    continue;
                }
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if seen.insert(canonical) {
                    files.push(path);
                }
            }
        }
    }

    files
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::commands::skill) enum AdmissionRisk {
    Safe,
    Suspicious,
    Malicious,
}

impl AdmissionRisk {
    pub(in crate::commands::skill) fn as_str(self) -> &'static str {
        match self {
            Self::Safe => "safe",
            Self::Suspicious => "suspicious",
            Self::Malicious => "malicious",
        }
    }

    #[cfg(feature = "agent")]
    fn from_cache_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "safe" => Self::Safe,
            "malicious" => Self::Malicious,
            _ => Self::Suspicious,
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::commands::skill) struct SkillScanReport {
    pub(in crate::commands::skill) name: String,
    pub(in crate::commands::skill) risk: AdmissionRisk,
    pub(in crate::commands::skill) messages: Vec<String>,
}

fn sample_scripts_for_llm(script_files: &[PathBuf], max_files: usize, max_chars: usize) -> String {
    let mut out = String::new();
    for script_path in script_files.iter().take(max_files) {
        let Ok(content) = fs::read_to_string(script_path) else {
            continue;
        };
        let snippet: String = content.chars().take(max_chars).collect();
        out.push_str(&format!(
            "\n### {}\n{}\n",
            script_path.display(),
            if content.chars().count() > max_chars {
                format!("{snippet}\n...<truncated>")
            } else {
                snippet
            }
        ));
    }
    out
}

#[cfg(feature = "agent")]
fn llm_admission_assess(skill_name: &str, skill_md: &str, script_samples: &str) -> Result<(AdmissionRisk, String)> {
    let config = AgentConfig::from_env();
    if config.api_key.trim().is_empty() {
        anyhow::bail!("LLM scan skipped: API key not configured");
    }

    // A3: Check scan cache to avoid redundant LLM calls for same content
    let content_hash = skilllite_core::scan_cache::content_hash(skill_md, script_samples);
    if let Some((risk_str, reason)) = skilllite_core::scan_cache::get_cached(&content_hash)? {
        return Ok((AdmissionRisk::from_cache_str(&risk_str), reason));
    }

    let system_prompt = r#"You are a security admission scanner for Skill packages.
Classify risk into one of: safe, suspicious, malicious.
Return STRICT JSON only:
{"risk":"safe|suspicious|malicious","reason":"...","evidence":["..."]}

Rules:
- malicious: direct harmful payload patterns, command execution abuse, obfuscated delivery, clear exploit intent.
- suspicious: potentially dangerous behaviors or unclear intent requiring manual review.
- safe: normal utility behavior with no obvious malicious intent.
"#;
    let user_prompt = format!(
        "Skill: {skill_name}\n\nSKILL.md:\n{skill_md}\n\nScript samples:\n{script_samples}"
    );

    let messages = vec![ChatMessage::system(system_prompt), ChatMessage::user(&user_prompt)];
    let client = LlmClient::new(&config.api_base, &config.api_key);
    let rt = tokio::runtime::Runtime::new().context("tokio runtime init failed")?;
    let resp = rt.block_on(async {
        tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client.chat_completion(&config.model, &messages, None, Some(0.1)),
        )
        .await
        .map_err(|_| anyhow::anyhow!("LLM request timed out (15s)"))?
    })?;
    let raw = resp
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_else(|| "{}".to_string());
    let cleaned = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let v: serde_json::Value = serde_json::from_str(cleaned)
        .with_context(|| format!("LLM risk JSON parse failed: {}", &raw.chars().take(180).collect::<String>()))?;
    let risk = match v.get("risk").and_then(|r| r.as_str()).unwrap_or("suspicious") {
        "safe" => AdmissionRisk::Safe,
        "malicious" => AdmissionRisk::Malicious,
        _ => AdmissionRisk::Suspicious,
    };
    let reason = v
        .get("reason")
        .and_then(|r| r.as_str())
        .unwrap_or("LLM flagged potential risk")
        .to_string();

    // A3: Store in scan cache for future lookups
    let _ = skilllite_core::scan_cache::put_cached(&content_hash, risk.as_str(), &reason);

    Ok((risk, reason))
}

#[cfg(not(feature = "agent"))]
fn llm_admission_assess(_skill_name: &str, _skill_md: &str, _script_samples: &str) -> Result<(AdmissionRisk, String)> {
    anyhow::bail!("LLM scan unavailable: binary built without `agent` feature")
}

pub(super) fn scan_candidate_skills(
    candidates: &[(String, PathBuf)],
    scan_offline: bool,
) -> Vec<SkillScanReport> {
    scan_candidate_skills_inner(candidates, scan_offline, scan_offline)
}

pub(in crate::commands::skill) fn scan_candidate_skills_fast(candidates: &[(String, PathBuf)]) -> Vec<SkillScanReport> {
    scan_candidate_skills_inner(candidates, true, false)
}

fn scan_candidate_skills_inner(
    candidates: &[(String, PathBuf)],
    skip_dep_audit: bool,
    scan_offline: bool,
) -> Vec<SkillScanReport> {
    let mut reports = Vec::new();

    let total = candidates.len();
    for (idx, (name, skill_path)) in candidates.iter().enumerate() {
        eprint!("   [{}/{}] {} ...", idx + 1, total, name);

        if !skill_path.join("SKILL.md").exists() {
            eprintln!(" ⚠ missing SKILL.md");
            reports.push(SkillScanReport {
                name: name.clone(),
                risk: AdmissionRisk::Suspicious,
                messages: vec![format!("   ⚠ {}: missing SKILL.md, treated as suspicious", name)],
            });
            continue;
        }

        let meta = match metadata::parse_skill_metadata(skill_path) {
            Ok(m) => m,
            Err(e) => {
                reports.push(SkillScanReport {
                    name: name.clone(),
                    risk: AdmissionRisk::Suspicious,
                    messages: vec![format!("   ⚠ {}: failed to parse SKILL.md ({})", name, e)],
                });
                continue;
            }
        };

        let mut risk = AdmissionRisk::Safe;
        let mut messages = Vec::new();
        let mut skill_md_content = String::new();

        let skill_md_path = skill_path.join("SKILL.md");
        if let Ok(content) = fs::read_to_string(&skill_md_path) {
            skill_md_content = content.clone();
            let alerts = skilllite_core::skill::skill_md_security::scan_skill_md_suspicious_patterns(&content);
            if !alerts.is_empty() {
                let high_count = alerts.iter().filter(|a| a.severity == "high").count();
                if high_count > 0 {
                    risk = AdmissionRisk::Malicious;
                } else {
                    risk = risk.max(AdmissionRisk::Suspicious);
                }
                messages.push(format!(
                    "   📄 {} SKILL.md: ⚠ {} alert(s) ({} high)",
                    name, alerts.len(), high_count
                ));
                for a in alerts.iter().take(3) {
                    messages.push(format!("      [{}] {}", a.severity.to_uppercase(), a.message));
                }
            }
        }

        let script_files = collect_script_files(skill_path, &meta);
        let has_deps = skill_path.join("requirements.txt").exists()
            || skill_path.join("package.json").exists()
            || skill_path.join(".skilllite.lock").exists()
            || meta.resolved_packages.is_some()
            || meta.compatibility.as_ref().map_or(false, |c| !c.is_empty());

        if script_files.is_empty() && !has_deps {
            let skill_type = if meta.is_bash_tool_skill() {
                "bash-tool"
            } else {
                "prompt-only"
            };
            messages.push(format!(
                "   ✅ {} ({}): no scripts or dependencies to scan",
                name, skill_type
            ));
        }

        if !script_files.is_empty() {
            let scanner = ScriptScanner::new();
            let mut total_issues = 0usize;
            let mut total_high = 0usize;
            let mut total_critical = 0usize;

            for script_path in &script_files {
                if let Ok(result) = scanner.scan_file(script_path) {
                    let high = result
                        .issues
                        .iter()
                        .filter(|i| matches!(i.severity, SecuritySeverity::High))
                        .count();
                    let critical = result
                        .issues
                        .iter()
                        .filter(|i| matches!(i.severity, SecuritySeverity::Critical))
                        .count();
                    total_issues += result.issues.len();
                    total_high += high;
                    total_critical += critical;
                }
            }

            if total_critical > 0 {
                risk = AdmissionRisk::Malicious;
            } else if total_high > 0 {
                risk = risk.max(AdmissionRisk::Suspicious);
            }
            if total_issues > 0 {
                messages.push(format!(
                    "   🔒 {} code scan: {} issue(s) across {} file(s) ({} high / {} critical)",
                    name, total_issues, script_files.len(), total_high, total_critical
                ));
            } else {
                messages.push(format!(
                    "   🔒 {} code scan: ✅ {} file(s) clean",
                    name, script_files.len()
                ));
            }
        }

        #[cfg(feature = "audit")]
        if has_deps && !skip_dep_audit && !scan_offline {
            use skilllite_sandbox::security::dependency_audit;

            let metadata_hint = metadata::parse_skill_metadata(skill_path)
                .ok()
                .map(crate::commands::metadata_into_hint);
            match dependency_audit::audit_skill_dependencies(skill_path, metadata_hint.as_ref()) {
                Ok(result) => {
                    if result.vulnerable_count > 0 {
                        risk = risk.max(AdmissionRisk::Suspicious);
                        messages.push(format!(
                            "   🛡 {} dependency audit: ⚠ {}/{} packages vulnerable ({} vulns)",
                            name, result.vulnerable_count, result.scanned, result.total_vulns
                        ));
                    } else if result.scanned > 0 {
                        messages.push(format!(
                            "   🛡 {} dependency audit: ✅ {} packages clean",
                            name, result.scanned
                        ));
                    }
                }
                Err(e) => {
                    risk = risk.max(AdmissionRisk::Suspicious);
                    messages.push(format!("   🛡 {} dependency audit: ⚠ error: {}", name, e));
                }
            }
        }

        let needs_llm = risk > AdmissionRisk::Safe && !scan_offline;
        if needs_llm {
            let script_samples = sample_scripts_for_llm(&script_files, 3, 1200);
            match llm_admission_assess(name, &skill_md_content, &script_samples) {
                Ok((llm_risk, reason)) => {
                    risk = risk.max(llm_risk);
                    messages.push(format!("   🧠 {} LLM confirm: {} ({})", name, llm_risk.as_str(), reason));
                }
                Err(e) => {
                    messages.push(format!("   🧠 {} LLM confirm skipped: {}", name, e));
                }
            }
        }

        eprintln!(" {}", risk.as_str());
        reports.push(SkillScanReport {
            name: name.clone(),
            risk,
            messages,
        });
    }

    reports
}
