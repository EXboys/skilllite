//! Smart dependency resolution (LLM → whitelist fallback).

use std::path::Path;

use super::config::make_agent;
use super::types::Dependency;

/// Pure data-driven dependency resolution (no `SkillMetadata` dependency).
pub(super) fn resolve_from_metadata_fields(
    skill_dir: &Path,
    compatibility: Option<&str>,
    resolved_packages: Option<&[String]>,
    description: Option<&str>,
    language_hint: Option<&str>,
    entry_point: &str,
) -> Vec<Dependency> {
    let compat = compatibility.unwrap_or("");
    if compat.is_empty() && resolved_packages.is_none() {
        return Vec::new();
    }

    let language = language_hint
        .map(String::from)
        .unwrap_or_else(|| detect_language_from_entry_point(entry_point, skill_dir));
    let ecosystem = match language.as_str() {
        "python" => "PyPI",
        "node" => "npm",
        _ => "PyPI",
    };

    if let Some(resolved) = resolved_packages {
        return resolved
            .iter()
            .map(|pkg| {
                if let Some((name, ver)) = pkg.split_once("==") {
                    Dependency {
                        name: name.trim().to_string(),
                        version: ver.trim().to_string(),
                        ecosystem: ecosystem.to_string(),
                    }
                } else {
                    Dependency {
                        name: pkg.trim().to_string(),
                        version: String::new(),
                        ecosystem: ecosystem.to_string(),
                    }
                }
            })
            .filter(|d| !d.name.is_empty())
            .collect();
    }

    let context = build_inference_context(description, compat);

    if let Some(packages) = infer_packages_with_llm(&context, &language) {
        if !packages.is_empty() {
            tracing::info!(
                "LLM inferred {} package(s): {}",
                packages.len(),
                packages.join(", ")
            );
            return packages
                .into_iter()
                .map(|name| Dependency {
                    name,
                    version: String::new(),
                    ecosystem: ecosystem.to_string(),
                })
                .collect();
        }
    }

    Vec::new()
}

/// Lightweight language detection for dependency audit.
pub(super) fn detect_language_from_entry_point(entry_point: &str, skill_dir: &Path) -> String {
    if entry_point.ends_with(".py") {
        return "python".to_string();
    }
    if entry_point.ends_with(".js") || entry_point.ends_with(".ts") {
        return "node".to_string();
    }
    if entry_point.ends_with(".sh") {
        return "bash".to_string();
    }
    let scripts_dir = skill_dir.join("scripts");
    if scripts_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension() {
                    match ext.to_string_lossy().as_ref() {
                        "py" => return "python".to_string(),
                        "js" | "ts" => return "node".to_string(),
                        "sh" => return "bash".to_string(),
                        _ => {}
                    }
                }
            }
        }
    }
    "python".to_string()
}

fn build_inference_context(description: Option<&str>, compatibility: &str) -> String {
    let mut parts = Vec::new();
    if let Some(desc) = description {
        parts.push(format!("Description: {}", desc));
    }
    if !compatibility.is_empty() {
        parts.push(format!("Compatibility: {}", compatibility));
    }
    let joined = parts.join("\n");
    joined.chars().take(2000).collect()
}

/// Use an OpenAI-compatible LLM to extract package names from skill metadata.
fn infer_packages_with_llm(context: &str, language: &str) -> Option<Vec<String>> {
    let cfg = skilllite_core::config::LlmConfig::try_from_env()?;
    let model = if cfg.model.is_empty() {
        "deepseek-chat".to_string()
    } else {
        cfg.model
    };

    let lang_label = if language == "python" {
        "Python (PyPI)"
    } else {
        "Node.js (npm)"
    };

    let prompt = format!(
        "From the following skill description, extract the {} package names that need \
         to be installed via pip/npm.\n\n\
         \"{}\"\n\n\
         Rules:\n\
         - Only return real, installable package names (e.g. 'pandas', 'numpy', 'tqdm').\n\
         - Do NOT include language runtimes (python, node, bash) or generic words \
           (library, network, access, internet).\n\
         - Do NOT include version specifiers.\n\
         - Return ONLY a JSON array of strings. No explanation.\n\
         Example: [\"pandas\", \"numpy\", \"tqdm\"]",
        lang_label, context
    );

    let agent = make_agent();
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": 0
    });

    let url = format!(
        "{}/chat/completions",
        cfg.api_base.trim_end_matches('/')
    );

    let response = agent
        .post(&url)
        .set("Authorization", &format!("Bearer {}", cfg.api_key))
        .set("Content-Type", "application/json")
        .send_json(&body)
        .ok()?;

    let result: serde_json::Value = response.into_json().ok()?;
    let content = result
        .get("choices")?
        .get(0)?
        .get("message")?
        .get("content")?
        .as_str()?;

    let content = content.trim();
    let cleaned = if content.starts_with("```") {
        content
            .lines()
            .filter(|l| !l.starts_with("```"))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        content.to_string()
    };

    let packages: Vec<String> = serde_json::from_str(cleaned.trim()).ok()?;

    let valid: Vec<String> = packages
        .into_iter()
        .map(|p| p.trim().to_lowercase())
        .filter(|p| !p.is_empty())
        .collect();

    if valid.is_empty() {
        None
    } else {
        Some(valid)
    }
}
