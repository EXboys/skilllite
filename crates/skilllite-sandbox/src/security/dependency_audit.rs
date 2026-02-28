//! Supply chain vulnerability scanning — multi-backend architecture.
//!
//! Parses dependency files (requirements.txt, package.json) from skill directories
//! and queries vulnerability databases for known issues.
//!
//! # Backend priority
//!
//! 1. **Custom API** (`SKILLLITE_AUDIT_API`): Your own security service endpoint.
//!    Uses OSV-compatible querybatch format, so any OSV-compatible backend works.
//!    Intended for commercial / enterprise deployments.
//!
//! 2. **PyPI JSON API** (Python packages only): Queries PyPI directly — the response
//!    includes a `vulnerabilities` field. Works out-of-the-box in mainland China
//!    via mirrors like `https://pypi.tuna.tsinghua.edu.cn`.
//!    Configurable via `PYPI_MIRROR_URL`.
//!
//! 3. **OSV.dev API** (npm, fallback): Batch query against Google's OSV database.
//!    Configurable via `OSV_API_URL`.
//!
//! # Environment variables
//!
//! | Variable | Default | Description |
//! |----------|---------|-------------|
//! | `SKILLLITE_AUDIT_API` | *(none)* | Custom security API (overrides all other backends) |
//! | `PYPI_MIRROR_URL` | `https://pypi.org` | PyPI mirror for Python vulnerability queries |
//! | `OSV_API_URL` | `https://api.osv.dev` | OSV API for npm / fallback queries |

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::malicious_packages::{check_malicious_packages, MaliciousPackageHit};

// ─── Types ──────────────────────────────────────────────────────────────────

/// A parsed dependency with name, version, and ecosystem.
#[derive(Debug, Clone, Serialize)]
pub struct Dependency {
    pub name: String,
    pub version: String,
    /// Ecosystem identifier: "PyPI" or "npm".
    pub ecosystem: String,
}

/// Vulnerability reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnRef {
    pub id: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub fixed_in: Vec<String>,
}

/// Audit entry for one package.
#[derive(Debug, Clone, Serialize)]
pub struct PackageAuditEntry {
    pub name: String,
    pub version: String,
    pub ecosystem: String,
    pub vulns: Vec<VulnRef>,
}

/// Which backend was used for the audit.
#[derive(Debug, Clone, Serialize)]
pub enum AuditBackend {
    /// Custom commercial API (SKILLLITE_AUDIT_API)
    Custom(String),
    /// PyPI JSON API (for Python) + OSV (for npm)
    Native,
}

/// Overall audit result.
#[derive(Debug, Clone, Serialize)]
pub struct DependencyAuditResult {
    pub scanned: usize,
    pub vulnerable_count: usize,
    pub total_vulns: usize,
    pub backend: AuditBackend,
    pub entries: Vec<PackageAuditEntry>,
    /// Packages matched by the offline malicious-package library (B4).
    /// Populated before any network call — zero-latency, zero-network.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub malicious: Vec<MaliciousPackageHit>,
}

/// Metadata hint for dependency inference when no explicit dependency files exist.
/// Provided by the commands layer (which parses SKILL.md); sandbox never parses skill metadata.
#[derive(Debug, Clone)]
pub struct MetadataHint {
    pub compatibility: Option<String>,
    pub resolved_packages: Option<Vec<String>>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub entry_point: String,
}

// ─── Dependency file parsers ────────────────────────────────────────────────

/// Parse Python `requirements.txt` / `pip freeze` output.
///
/// Handles `package==version` (exact), `package>=version`, `package~=version`.
/// Lines without a version constraint are skipped.
pub fn parse_requirements_txt(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        let line = line.split('#').next().unwrap_or(line).trim();

        if let Some((name, version)) = line.split_once("==") {
            push_if_valid(&mut deps, name, version, "PyPI");
            continue;
        }
        if let Some(idx) = line.find(|c: char| matches!(c, '>' | '<' | '~' | '!')) {
            let name = &line[..idx];
            let rest = &line[idx..];
            let version = rest
                .trim_start_matches(|c: char| matches!(c, '>' | '<' | '~' | '!' | '='));
            let version = version.split(',').next().unwrap_or("").trim();
            push_if_valid(&mut deps, name, version, "PyPI");
        }
    }
    deps
}

/// Parse Node.js `package.json` dependencies.
///
/// Reads `dependencies` and `devDependencies`. Strips `^`, `~`, `>=` prefixes
/// to extract a base version suitable for OSV queries.
pub fn parse_package_json(content: &str) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let parsed: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return deps,
    };

    for section in &["dependencies", "devDependencies"] {
        if let Some(obj) = parsed.get(section).and_then(|v| v.as_object()) {
            for (name, version_val) in obj {
                if let Some(version_str) = version_val.as_str() {
                    let version = version_str
                        .trim_start_matches('^')
                        .trim_start_matches('~')
                        .trim_start_matches(">=")
                        .trim_start_matches('>')
                        .trim_start_matches("<=")
                        .trim_start_matches('<')
                        .trim_start_matches('=')
                        .trim();
                    if version.is_empty()
                        || version.starts_with("http")
                        || version.starts_with("git")
                        || version.contains('/')
                        || version == "*"
                        || version == "latest"
                    {
                        continue;
                    }
                    deps.push(Dependency {
                        name: name.clone(),
                        version: version.to_string(),
                        ecosystem: "npm".to_string(),
                    });
                }
            }
        }
    }
    deps
}

/// Collect all dependencies from a skill directory.
///
/// Sources checked (in priority order):
/// 1. `requirements.txt` (Python, explicit)
/// 2. `package.json` (Node.js, explicit)
/// 3. `.skilllite.lock` → `resolved_packages` (written by `skilllite init`)
/// 4. **Smart inference** — when `metadata_hint` is provided and no explicit files found:
///    LLM inference → whitelist fallback (uses compatibility, resolved_packages, etc.)
///
/// `metadata_hint`: When `Some`, used for inference when no dependency files exist.
/// Must be provided by the commands layer (which parses SKILL.md); sandbox never parses skill metadata.
///
/// Deduplicates by (name, ecosystem) — explicit files take priority over lock file.
pub fn collect_dependencies(skill_dir: &Path, metadata_hint: Option<&MetadataHint>) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. Python: requirements.txt
    let req_txt = skill_dir.join("requirements.txt");
    if req_txt.exists() {
        if let Ok(content) = std::fs::read_to_string(&req_txt) {
            for dep in parse_requirements_txt(&content) {
                seen.insert((dep.name.to_lowercase(), dep.ecosystem.clone()));
                deps.push(dep);
            }
        }
    }

    // 2. Node.js: package.json
    let pkg_json = skill_dir.join("package.json");
    if pkg_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            for dep in parse_package_json(&content) {
                seen.insert((dep.name.to_lowercase(), dep.ecosystem.clone()));
                deps.push(dep);
            }
        }
    }

    // 3. .skilllite.lock: resolved_packages (pip freeze format: "package==version")
    let lock_path = skill_dir.join(".skilllite.lock");
    if lock_path.exists() {
        if let Some(lock_deps) = parse_lock_file(&lock_path) {
            for dep in lock_deps {
                let key = (dep.name.to_lowercase(), dep.ecosystem.clone());
                if !seen.contains(&key) {
                    seen.insert(key);
                    deps.push(dep);
                }
            }
        }
    }

    // 4. No explicit files found and metadata_hint provided → infer via LLM/whitelist
    if deps.is_empty() {
        if let Some(hint) = metadata_hint {
            let inferred = resolve_from_metadata_fields(
                skill_dir,
                hint.compatibility.as_deref(),
                hint.resolved_packages.as_deref(),
                hint.description.as_deref(),
                hint.language.as_deref(),
                &hint.entry_point,
            );
            for dep in inferred {
                let key = (dep.name.to_lowercase(), dep.ecosystem.clone());
                if !seen.contains(&key) {
                    seen.insert(key);
                    deps.push(dep);
                }
            }
        }
    }

    deps
}

/// Parse `.skilllite.lock` JSON for resolved packages.
///
/// The lock file is written by `skilllite init` and contains:
/// ```json
/// { "resolved_packages": ["requests==2.31.0", "flask==3.0.0"], ... }
/// ```
fn parse_lock_file(lock_path: &Path) -> Option<Vec<Dependency>> {
    let content = std::fs::read_to_string(lock_path).ok()?;
    let lock: serde_json::Value = serde_json::from_str(&content).ok()?;
    let arr = lock.get("resolved_packages")?.as_array()?;

    let packages: Vec<String> = arr
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    if packages.is_empty() {
        return None;
    }

    // resolved_packages are in pip freeze format ("name==version")
    // Reuse the requirements.txt parser
    let fake_requirements = packages.join("\n");
    let deps = parse_requirements_txt(&fake_requirements);

    if deps.is_empty() { None } else { Some(deps) }
}

// ─── Smart Dependency Resolution (LLM → whitelist fallback) ─────────────────

/// Pure data-driven dependency resolution (no `SkillMetadata` dependency).
fn resolve_from_metadata_fields(
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

    let whitelist_packages =
        skilllite_core::skill::deps::parse_compatibility_for_packages(Some(compat));
    if !whitelist_packages.is_empty() {
        tracing::info!(
            "Whitelist matched {} package(s): {}",
            whitelist_packages.len(),
            whitelist_packages.join(", ")
        );
        return whitelist_packages
            .into_iter()
            .map(|name| Dependency {
                name,
                version: String::new(),
                ecosystem: ecosystem.to_string(),
            })
            .collect();
    }

    Vec::new()
}

/// Lightweight language detection for dependency audit (avoids importing skill::metadata).
fn detect_language_from_entry_point(entry_point: &str, skill_dir: &Path) -> String {
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

/// Build a concise context string for LLM inference.
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
///
/// Environment variables:
/// - `OPENAI_API_BASE` or `BASE_URL`: API endpoint
/// - `OPENAI_API_KEY` or `API_KEY`: Authentication key
/// - `SKILLLITE_MODEL` or `MODEL`: Model name (default: "deepseek-chat")
///
/// Returns `None` if LLM is not configured or the call fails.
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

    // Strip markdown code fences if present
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

// ─── Configuration ──────────────────────────────────────────────────────────

const DEFAULT_PYPI_BASE: &str = "https://pypi.org";
const DEFAULT_OSV_API_BASE: &str = "https://api.osv.dev";

/// Get custom audit API URL, if configured.
fn get_custom_api() -> Option<String> {
    skilllite_core::config::load_dotenv();
    skilllite_core::config::env_optional(
        skilllite_core::config::env_keys::misc::SKILLLITE_AUDIT_API,
        &[],
    )
    .map(|s| s.trim_end_matches('/').to_string())
}

/// Get PyPI mirror base URL.
fn get_pypi_base() -> String {
    skilllite_core::config::load_dotenv();
    skilllite_core::config::env_or(
        skilllite_core::config::env_keys::misc::PYPI_MIRROR_URL,
        &[],
        || DEFAULT_PYPI_BASE.to_string(),
    )
    .trim_end_matches('/')
    .to_string()
}

/// Get OSV API base URL.
fn get_osv_api_base() -> String {
    skilllite_core::config::load_dotenv();
    skilllite_core::config::env_or(
        skilllite_core::config::env_keys::misc::OSV_API_URL,
        &[],
        || DEFAULT_OSV_API_BASE.to_string(),
    )
    .trim_end_matches('/')
    .to_string()
}

fn make_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(30))
        .build()
}

// ─── Backend 1: Custom commercial API (OSV-compatible) ──────────────────────

/// OSV batch response structure (shared with custom API).
#[derive(Deserialize)]
struct OsvBatchResponse {
    results: Vec<OsvQueryResult>,
}

#[derive(Deserialize)]
struct OsvQueryResult {
    #[serde(default)]
    vulns: Vec<OsvVulnRef>,
}

#[derive(Deserialize)]
struct OsvVulnRef {
    id: String,
    #[serde(default)]
    #[allow(dead_code)]
    modified: String,
    #[serde(default)]
    summary: String,
}

/// Query an OSV-compatible batch API (custom or osv.dev).
fn query_osv_batch(
    agent: &ureq::Agent,
    deps: &[Dependency],
    api_base: &str,
) -> Result<Vec<PackageAuditEntry>> {
    let batch_url = format!("{}/v1/querybatch", api_base);
    let mut entries = Vec::new();

    for chunk in deps.chunks(100) {
        let queries: Vec<serde_json::Value> = chunk
            .iter()
            .map(|d| {
                serde_json::json!({
                    "package": { "name": d.name, "ecosystem": d.ecosystem },
                    "version": d.version,
                })
            })
            .collect();

        let body = serde_json::json!({ "queries": queries });

        let response = agent
            .post(&batch_url)
            .send_json(&body)
            .map_err(|e| match &e {
                ureq::Error::Status(code, _) => {
                    anyhow::anyhow!("Audit API returned HTTP {} — {}", code, e)
                }
                ureq::Error::Transport(_) => {
                    anyhow::anyhow!(
                        "Cannot reach audit API at {} : {}",
                        api_base, e
                    )
                }
            })?;

        let batch: OsvBatchResponse = response
            .into_json()
            .context("Failed to parse audit API response")?;

        for (dep, result) in chunk.iter().zip(batch.results.into_iter()) {
            entries.push(PackageAuditEntry {
                name: dep.name.clone(),
                version: dep.version.clone(),
                ecosystem: dep.ecosystem.clone(),
                vulns: result
                    .vulns
                    .into_iter()
                    .map(|v| VulnRef {
                        id: v.id,
                        summary: v.summary,
                        fixed_in: Vec::new(),
                    })
                    .collect(),
            });
        }
    }

    Ok(entries)
}

// ─── Backend 2: PyPI JSON API ───────────────────────────────────────────────

/// PyPI JSON API response (subset).
#[derive(Deserialize)]
struct PypiResponse {
    #[serde(default)]
    vulnerabilities: Vec<PypiVuln>,
}

#[derive(Deserialize)]
struct PypiVuln {
    #[serde(default)]
    id: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    fixed_in: Vec<String>,
}

/// PyPI JSON API response for latest version (no version in URL).
/// Includes `info.version` to resolve the actual version number.
#[derive(Deserialize)]
struct PypiInfoResponse {
    #[serde(default)]
    info: PypiInfo,
    #[serde(default)]
    vulnerabilities: Vec<PypiVuln>,
}

#[derive(Deserialize, Default)]
struct PypiInfo {
    #[serde(default)]
    version: String,
}

/// Query PyPI JSON API for vulnerabilities on a list of Python packages.
///
/// - With version:  `GET {mirror}/pypi/{name}/{version}/json`
/// - Without version: `GET {mirror}/pypi/{name}/json` (latest)
///
/// PyPI returns a `vulnerabilities` field with pre-matched results —
/// no version comparison logic needed on our side.
fn query_pypi(
    agent: &ureq::Agent,
    deps: &[Dependency],
    pypi_base: &str,
) -> Result<Vec<PackageAuditEntry>> {
    let mut entries = Vec::new();

    for dep in deps {
        // Choose URL based on whether we have a version
        let (url, has_version) = if dep.version.is_empty() {
            (format!("{}/pypi/{}/json", pypi_base, dep.name), false)
        } else {
            (
                format!("{}/pypi/{}/{}/json", pypi_base, dep.name, dep.version),
                true,
            )
        };

        let result = agent.get(&url).call();
        match result {
            Ok(response) => {
                if has_version {
                    // Versioned response — simple structure
                    let pypi: PypiResponse = response
                        .into_json()
                        .unwrap_or(PypiResponse { vulnerabilities: Vec::new() });

                    entries.push(PackageAuditEntry {
                        name: dep.name.clone(),
                        version: dep.version.clone(),
                        ecosystem: dep.ecosystem.clone(),
                        vulns: pypi
                            .vulnerabilities
                            .into_iter()
                            .map(|v| VulnRef {
                                id: v.id,
                                summary: v.summary,
                                fixed_in: v.fixed_in,
                            })
                            .collect(),
                    });
                } else {
                    // Versionless response — extract latest version from info
                    let pypi: PypiInfoResponse = response.into_json().unwrap_or(
                        PypiInfoResponse {
                            info: PypiInfo { version: "latest".to_string() },
                            vulnerabilities: Vec::new(),
                        },
                    );

                    let resolved_version = if pypi.info.version.is_empty() {
                        "latest".to_string()
                    } else {
                        pypi.info.version
                    };

                    entries.push(PackageAuditEntry {
                        name: dep.name.clone(),
                        version: resolved_version,
                        ecosystem: dep.ecosystem.clone(),
                        vulns: pypi
                            .vulnerabilities
                            .into_iter()
                            .map(|v| VulnRef {
                                id: v.id,
                                summary: v.summary,
                                fixed_in: v.fixed_in,
                            })
                            .collect(),
                    });
                }
            }
            Err(ureq::Error::Status(404, _)) => {
                // Package/version not found on PyPI — skip silently
                let version = if dep.version.is_empty() {
                    "unknown".to_string()
                } else {
                    dep.version.clone()
                };
                entries.push(PackageAuditEntry {
                    name: dep.name.clone(),
                    version,
                    ecosystem: dep.ecosystem.clone(),
                    vulns: Vec::new(),
                });
            }
            Err(e) => {
                let version_display = if dep.version.is_empty() {
                    "latest"
                } else {
                    &dep.version
                };
                tracing::warn!(
                    "Failed to query PyPI for {} {}: {}",
                    dep.name, version_display, e
                );
                entries.push(PackageAuditEntry {
                    name: dep.name.clone(),
                    version: dep.version.clone(),
                    ecosystem: dep.ecosystem.clone(),
                    vulns: Vec::new(),
                });
            }
        }
    }

    Ok(entries)
}

// ─── Public entry point ─────────────────────────────────────────────────────

/// Run a full dependency audit on a skill directory.
///
/// `metadata_hint`: When `Some`, used for inference when no explicit dependency
/// files exist. Must be provided by the commands layer (which parses SKILL.md);
/// sandbox never parses skill metadata.
///
/// Backend selection:
/// 1. If `SKILLLITE_AUDIT_API` is set → all packages via custom API
/// 2. Otherwise → Python via PyPI JSON API, npm via OSV batch API
pub fn audit_skill_dependencies(
    skill_dir: &Path,
    metadata_hint: Option<&MetadataHint>,
) -> Result<DependencyAuditResult> {
    let deps = collect_dependencies(skill_dir, metadata_hint);

    // ── B4: Offline malicious-package check (zero-network, instant) ──────────
    // Run BEFORE any network call so installs are blocked even in --scan-offline mode.
    let malicious_hits = check_malicious_packages(
        deps.iter().map(|d| (d.name.as_str(), d.ecosystem.as_str())),
    );
    if !malicious_hits.is_empty() {
        for hit in &malicious_hits {
            tracing::warn!(
                "🔴 Malicious package detected (offline DB): {} [{}] — {}",
                hit.name,
                hit.ecosystem,
                hit.reason
            );
        }
    }

    if deps.is_empty() {
        return Ok(DependencyAuditResult {
            scanned: 0,
            vulnerable_count: 0,
            total_vulns: 0,
            backend: AuditBackend::Native,
            entries: Vec::new(),
            malicious: malicious_hits,
        });
    }

    let agent = make_agent();

    // Backend 1: Custom commercial API (highest priority)
    if let Some(custom_url) = get_custom_api() {
        tracing::info!(
            "Scanning {} dependencies via custom API ({})...",
            deps.len(),
            custom_url
        );
        let entries = query_osv_batch(&agent, &deps, &custom_url)?;
        return Ok(build_result(entries, AuditBackend::Custom(custom_url), malicious_hits));
    }

    // Backend 2+3: Native — PyPI for Python, OSV for npm
    let pypi_deps: Vec<_> = deps.iter().filter(|d| d.ecosystem == "PyPI").cloned().collect();
    let npm_deps: Vec<_> = deps.iter().filter(|d| d.ecosystem == "npm").cloned().collect();

    let mut all_entries = Vec::new();

    // Python packages → PyPI JSON API
    if !pypi_deps.is_empty() {
        let pypi_base = get_pypi_base();
        let mirror_note = if pypi_base != DEFAULT_PYPI_BASE {
            format!(" (via {})", pypi_base)
        } else {
            String::new()
        };
        tracing::info!(
            "Scanning {} Python dependencies via PyPI{}...",
            pypi_deps.len(),
            mirror_note
        );
        let pypi_entries = query_pypi(&agent, &pypi_deps, &pypi_base)?;
        all_entries.extend(pypi_entries);
    }

    // npm packages → OSV batch API
    if !npm_deps.is_empty() {
        let osv_base = get_osv_api_base();
        let mirror_note = if osv_base != DEFAULT_OSV_API_BASE {
            format!(" (via {})", osv_base)
        } else {
            String::new()
        };
        tracing::info!(
            "Scanning {} npm dependencies via OSV{}...",
            npm_deps.len(),
            mirror_note
        );
        let osv_entries = query_osv_batch(&agent, &npm_deps, &osv_base)?;
        all_entries.extend(osv_entries);
    }

    Ok(build_result(all_entries, AuditBackend::Native, malicious_hits))
}

fn build_result(
    entries: Vec<PackageAuditEntry>,
    backend: AuditBackend,
    malicious: Vec<MaliciousPackageHit>,
) -> DependencyAuditResult {
    let vulnerable_count = entries.iter().filter(|e| !e.vulns.is_empty()).count();
    let total_vulns: usize = entries.iter().map(|e| e.vulns.len()).sum();
    let scanned = entries.len();
    DependencyAuditResult {
        scanned,
        vulnerable_count,
        total_vulns,
        backend,
        entries,
        malicious,
    }
}

// ─── Formatting ─────────────────────────────────────────────────────────────

/// Format audit result for human-readable terminal display.
pub fn format_audit_result(result: &DependencyAuditResult) -> String {
    let mut lines: Vec<String> = Vec::new();

    // ── B4: Offline malicious-package hits (shown first, highest priority) ───
    if !result.malicious.is_empty() {
        lines.push(format!(
            "🔴 Malicious Package Library: {} known-malicious package(s) detected!",
            result.malicious.len()
        ));
        lines.push(String::new());
        for hit in &result.malicious {
            lines.push(format!(
                "  ☠️  {} [{}]",
                hit.name, hit.ecosystem
            ));
            lines.push(format!("     └─ {}", hit.reason));
        }
        lines.push(String::new());
    }

    if result.scanned == 0 {
        if result.malicious.is_empty() {
            return "ℹ  No dependencies detected (no files, lock, or inferred packages).".to_string();
        }
        return lines.join("\n");
    }

    if result.vulnerable_count == 0 && result.malicious.is_empty() {
        return format!(
            "✅ Scanned {} dependencies — no known vulnerabilities found.",
            result.scanned
        );
    }

    if result.vulnerable_count == 0 {
        lines.push(format!(
            "✅ Scanned {} dependencies — no known CVE vulnerabilities found.",
            result.scanned
        ));
        return lines.join("\n");
    }

    lines.push(format!(
        "⚠️  Supply Chain Audit: {}/{} packages have known vulnerabilities ({} total)",
        result.vulnerable_count, result.scanned, result.total_vulns
    ));
    lines.push(String::new());

    for entry in &result.entries {
        if entry.vulns.is_empty() {
            continue;
        }
        lines.push(format!(
            "  🔴 {} {} [{}]",
            entry.name, entry.version, entry.ecosystem
        ));
        for vuln in entry.vulns.iter().take(10) {
            let fix = if vuln.fixed_in.is_empty() {
                String::new()
            } else {
                format!(" → fix: {}", vuln.fixed_in.join(", "))
            };
            let summary = if vuln.summary.is_empty() {
                String::new()
            } else {
                let s = if vuln.summary.len() > 60 {
                    format!("{}...", &vuln.summary[..57])
                } else {
                    vuln.summary.clone()
                };
                format!(" — {}", s)
            };
            lines.push(format!(
                "     └─ {}{}{}",
                vuln.id, summary, fix
            ));
        }
        if entry.vulns.len() > 10 {
            lines.push(format!(
                "     ... and {} more",
                entry.vulns.len() - 10
            ));
        }
        lines.push(String::new());
    }

    let tip = match &result.backend {
        AuditBackend::Custom(url) => format!("🔗 Scanned via custom API: {}", url),
        AuditBackend::Native => "💡 Visit https://osv.dev/vulnerability/<ID> for details.".to_string(),
    };
    lines.push(tip);

    lines.join("\n")
}

/// Format audit result as structured JSON.
pub fn format_audit_result_json(result: &DependencyAuditResult) -> String {
    serde_json::to_string_pretty(result).unwrap_or_else(|_| "{}".to_string())
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn push_if_valid(deps: &mut Vec<Dependency>, name: &str, version: &str, ecosystem: &str) {
    let name = name.trim();
    let version = version.trim();
    if !name.is_empty() && !version.is_empty() {
        deps.push(Dependency {
            name: name.to_string(),
            version: version.to_string(),
            ecosystem: ecosystem.to_string(),
        });
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_requirements_txt_exact() {
        let content = "requests==2.31.0\nflask==3.0.0\n";
        let deps = parse_requirements_txt(content);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].name, "requests");
        assert_eq!(deps[0].version, "2.31.0");
        assert_eq!(deps[0].ecosystem, "PyPI");
        assert_eq!(deps[1].name, "flask");
        assert_eq!(deps[1].version, "3.0.0");
    }

    #[test]
    fn test_parse_requirements_txt_operators() {
        let content = "requests>=2.25.0\nflask~=2.0\nnumpy<2.0\n";
        let deps = parse_requirements_txt(content);
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].version, "2.25.0");
        assert_eq!(deps[1].version, "2.0");
        assert_eq!(deps[2].version, "2.0");
    }

    #[test]
    fn test_parse_requirements_txt_skip_comments_and_flags() {
        let content = "# comment\n-r other.txt\n-e git+https://...\nrequests==1.0\n";
        let deps = parse_requirements_txt(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "requests");
    }

    #[test]
    fn test_parse_requirements_txt_inline_comment() {
        let content = "requests==2.31.0  # HTTP library\n";
        let deps = parse_requirements_txt(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].version, "2.31.0");
    }

    #[test]
    fn test_parse_package_json() {
        let content = r#"{
            "dependencies": {
                "express": "^4.18.2",
                "lodash": "~4.17.21"
            },
            "devDependencies": {
                "jest": ">=29.0.0"
            }
        }"#;
        let deps = parse_package_json(content);
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[0].ecosystem, "npm");
        assert!(deps.iter().any(|d| d.name == "express" && d.version == "4.18.2"));
        assert!(deps.iter().any(|d| d.name == "lodash" && d.version == "4.17.21"));
        assert!(deps.iter().any(|d| d.name == "jest" && d.version == "29.0.0"));
    }

    #[test]
    fn test_parse_package_json_skip_non_versions() {
        let content = r#"{
            "dependencies": {
                "my-lib": "git+https://github.com/foo/bar",
                "other": "*",
                "valid": "1.0.0"
            }
        }"#;
        let deps = parse_package_json(content);
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].name, "valid");
    }

    #[test]
    fn test_backend_priority_custom_api() {
        // When SKILLLITE_AUDIT_API is set, get_custom_api returns Some
        // (actual env var test — we just verify the function logic)
        assert!(get_custom_api().is_none()); // not set in test env
    }

    #[test]
    fn test_build_result_counts() {
        let entries = vec![
            PackageAuditEntry {
                name: "a".into(),
                version: "1.0".into(),
                ecosystem: "PyPI".into(),
                vulns: vec![VulnRef {
                    id: "V-1".into(),
                    summary: "test".into(),
                    fixed_in: vec!["1.1".into()],
                }],
            },
            PackageAuditEntry {
                name: "b".into(),
                version: "2.0".into(),
                ecosystem: "npm".into(),
                vulns: vec![],
            },
        ];
        let result = build_result(entries, AuditBackend::Native, vec![]);
        assert_eq!(result.scanned, 2);
        assert_eq!(result.vulnerable_count, 1);
        assert_eq!(result.total_vulns, 1);
        assert!(result.malicious.is_empty());
    }
}
