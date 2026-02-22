//! Async dependency resolution with LLM inference.
//!
//! Extends `skilllite_core::skill::dependency_resolver` with LLM-based package extraction.
//! Used by `skilllite init` when an LLM client is available.

use anyhow::Result;
use skilllite_core::skill::dependency_resolver::{
    resolve_from_lock, resolve_from_whitelist, validate_against_whitelist, write_lock,
    ResolvedDependencies, ResolverKind,
};
use std::path::Path;

use crate::llm::LlmClient;
use crate::types::ChatMessage;

/// Use LLM to extract package names from compatibility string, then verify
/// each against the real package registry (PyPI / npm).
pub async fn resolve_from_llm(
    llm_client: &LlmClient,
    model: &str,
    compatibility: &str,
    language: &str,
) -> Option<Vec<String>> {
    let prompt = format!(
        "Extract the exact installable package names from this compatibility string.\n\
         Language: {}\n\
         Compatibility: \"{}\"\n\n\
         Rules:\n\
         - Only return package names that can be installed via pip (Python) or npm (Node.js).\n\
         - Do NOT include standard library modules (os, sys, json, etc.).\n\
         - Do NOT include language runtimes (Python, Node.js).\n\
         - Do NOT include system tools (git, docker, etc.).\n\
         - Return one package name per line, nothing else.\n\
         - If no installable packages, return NONE.\n\n\
         Output:",
        language, compatibility
    );

    let messages = vec![ChatMessage::user(&prompt)];
    let resp = llm_client
        .chat_completion(model, &messages, None, Some(0.0))
        .await
        .ok()?;
    let text = resp.choices.first()?.message.content.as_ref()?;

    if text.trim().eq_ignore_ascii_case("NONE") || text.trim().is_empty() {
        return None;
    }

    let candidates: Vec<String> = text
        .lines()
        .map(|l| {
            l.trim().trim_matches(
                |c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.',
            )
        })
        .filter(|l| !l.is_empty())
        .map(|l| l.to_lowercase())
        .collect();

    if candidates.is_empty() {
        return None;
    }

    let mut verified = Vec::new();
    for pkg in &candidates {
        if verify_package(pkg, language).await {
            verified.push(pkg.clone());
        } else {
            tracing::debug!("LLM-suggested package '{}' failed verification", pkg);
        }
    }

    if verified.is_empty() {
        None
    } else {
        Some(verified)
    }
}

async fn verify_package(name: &str, language: &str) -> bool {
    let url = match language {
        "python" => format!("https://pypi.org/pypi/{}/json", name),
        "node" => format!("https://registry.npmjs.org/{}", name),
        _ => return false,
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(_) => return false,
    };

    match client.head(&url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Full async resolution: Lock → LLM → Whitelist.
pub async fn resolve_packages(
    skill_dir: &Path,
    compatibility: Option<&str>,
    language: &str,
    llm_client: Option<&LlmClient>,
    model: Option<&str>,
    allow_unknown: bool,
) -> Result<ResolvedDependencies> {
    if let Some(packages) = resolve_from_lock(skill_dir, compatibility) {
        tracing::debug!("Resolved from lock: {:?}", packages);
        return Ok(ResolvedDependencies {
            packages,
            resolver: ResolverKind::Lock,
            unknown_packages: Vec::new(),
        });
    }

    let compat_str = compatibility.unwrap_or("");

    if !compat_str.is_empty() {
        if let (Some(client), Some(model)) = (llm_client, model) {
            match resolve_from_llm(client, model, compat_str, language).await {
                Some(packages) if !packages.is_empty() => {
                    let unknown = if allow_unknown {
                        Vec::new()
                    } else {
                        validate_against_whitelist(&packages, language)
                    };

                    let _ = write_lock(
                        skill_dir,
                        compatibility,
                        language,
                        &packages,
                        &ResolverKind::Llm,
                    );

                    return Ok(ResolvedDependencies {
                        packages,
                        resolver: ResolverKind::Llm,
                        unknown_packages: unknown,
                    });
                }
                _ => {
                    tracing::debug!("LLM inference returned no packages, falling through");
                }
            }
        }
    }

    if !compat_str.is_empty() {
        let packages = resolve_from_whitelist(compat_str, language);
        if !packages.is_empty() {
            let unknown = if allow_unknown {
                Vec::new()
            } else {
                validate_against_whitelist(&packages, language)
            };

            let _ = write_lock(
                skill_dir,
                compatibility,
                language,
                &packages,
                &ResolverKind::Whitelist,
            );

            return Ok(ResolvedDependencies {
                packages,
                resolver: ResolverKind::Whitelist,
                unknown_packages: unknown,
            });
        }
    }

    Ok(ResolvedDependencies {
        packages: Vec::new(),
        resolver: ResolverKind::None,
        unknown_packages: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_packages_no_llm_falls_to_whitelist() {
        let td = tempfile::tempdir().unwrap();
        let skill_dir = td.path();
        let res = resolve_packages(
            skill_dir,
            Some("Requires Python 3.x with requests"),
            "python",
            None,
            None,
            true,
        )
        .await;
        assert!(res.is_ok());
        let r = res.unwrap();
        assert_eq!(r.resolver, ResolverKind::Whitelist);
        assert!(r.packages.contains(&"requests".to_string()));
    }
}
