//! Dependency resolution pipeline: Lock → LLM → Whitelist.
//!
//! Three-layer resolution for skill dependencies extracted from the `compatibility`
//! field in SKILL.md:
//!
//!   ① Lock file (`.skilllite.lock`) — fast cache hit (sync)
//!   ② LLM inference — call LLM to extract package names, verify via PyPI/npm (async)
//!   ③ Whitelist matching — tokenize compatibility string, match against known packages (sync)
//!
//! After resolution, packages can optionally be validated against the whitelist
//! (`--allow-unknown-packages` bypasses this check).
//!
//! Ported from Python `core/dependency_resolver.py`.

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path;

// ─── Result type ─────────────────────────────────────────────────────────────

/// Result of the dependency resolution pipeline.
#[derive(Debug, Clone)]
pub struct ResolvedDependencies {
    /// Resolved package names (pip/npm installable).
    pub packages: Vec<String>,
    /// Which resolver layer produced the result.
    pub resolver: ResolverKind,
    /// Packages not found in the whitelist (non-empty only when validation runs).
    pub unknown_packages: Vec<String>,
}

/// Which resolver layer produced the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolverKind {
    Lock,
    Llm,
    Whitelist,
    None,
}

impl std::fmt::Display for ResolverKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lock => write!(f, "lock"),
            Self::Llm => write!(f, "llm"),
            Self::Whitelist => write!(f, "whitelist"),
            Self::None => write!(f, "none"),
        }
    }
}

// ─── Lock file layer (① fast path) ──────────────────────────────────────────

/// Read `.skilllite.lock` and return cached packages if fresh.
pub fn resolve_from_lock(skill_dir: &Path, compatibility: Option<&str>) -> Option<Vec<String>> {
    let lock_path = skill_dir.join(".skilllite.lock");
    let content = std::fs::read_to_string(&lock_path).ok()?;
    let lock: serde_json::Value = serde_json::from_str(&content).ok()?;

    let current_hash = compatibility_hash(compatibility);
    if lock.get("compatibility_hash")?.as_str()? != current_hash {
        tracing::debug!("Lock file stale: hash mismatch");
        return None;
    }

    let arr = lock.get("resolved_packages")?.as_array()?;
    let packages: Vec<String> = arr.iter().filter_map(|v| v.as_str().map(String::from)).collect();
    if packages.is_empty() {
        None
    } else {
        Some(packages)
    }
}

/// Write a fresh `.skilllite.lock`.
pub fn write_lock(
    skill_dir: &Path,
    compatibility: Option<&str>,
    language: &str,
    packages: &[String],
    resolver: &ResolverKind,
) -> Result<()> {
    let mut sorted = packages.to_vec();
    sorted.sort();

    let lock = serde_json::json!({
        "compatibility_hash": compatibility_hash(compatibility),
        "language": language,
        "resolved_packages": sorted,
        "resolved_at": chrono::Utc::now().to_rfc3339(),
        "resolver": resolver.to_string(),
    });

    let lock_path = skill_dir.join(".skilllite.lock");
    std::fs::write(&lock_path, serde_json::to_string_pretty(&lock)? + "\n")?;
    Ok(())
}

fn compatibility_hash(compat: Option<&str>) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(compat.unwrap_or("").as_bytes());
    hex::encode(h.finalize())
}

// ─── LLM inference layer (② cold path) ──────────────────────────────────────

/// Use LLM to extract package names from compatibility string, then verify
/// each against the real package registry (PyPI / npm).
///
/// Only available when the `agent` feature is enabled (requires `reqwest`).
#[cfg(feature = "agent")]
pub async fn resolve_from_llm(
    llm_client: &crate::agent::llm::LlmClient,
    model: &str,
    compatibility: &str,
    language: &str,
) -> Option<Vec<String>> {
    use crate::agent::types::ChatMessage;

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
    let resp = llm_client.chat_completion(model, &messages, None, Some(0.0)).await.ok()?;
    let text = resp.choices.first()?.message.content.as_ref()?;

    if text.trim().eq_ignore_ascii_case("NONE") || text.trim().is_empty() {
        return None;
    }

    let candidates: Vec<String> = text
        .lines()
        .map(|l| l.trim().trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_' && c != '.'))
        .filter(|l| !l.is_empty())
        .map(|l| l.to_lowercase())
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // Verify each candidate against the real registry
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

/// Verify a package exists on PyPI or npm.
#[cfg(feature = "agent")]
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

// ─── Whitelist matching layer (③ offline fallback) ───────────────────────────

/// Extract packages from compatibility string by matching against the embedded whitelist.
pub fn resolve_from_whitelist(compatibility: &str, language: &str) -> Vec<String> {
    let whitelist = get_whitelist();
    let compat_lower = compatibility.to_lowercase();

    let (packages, aliases) = match language {
        "python" => (&whitelist.python_packages, &whitelist.python_aliases),
        "node" => (&whitelist.node_packages, &whitelist.node_aliases),
        _ => return Vec::new(),
    };

    let mut matched = Vec::new();

    // Check direct package names
    for pkg in packages {
        if is_word_boundary_match(&compat_lower, &pkg.to_lowercase()) {
            matched.push(pkg.clone());
        }
    }

    // Check aliases (e.g. "cv2" → "opencv-python", "PIL" → "pillow")
    for (alias, canonical) in aliases {
        if is_word_boundary_match(&compat_lower, &alias.to_lowercase()) {
            if !matched.contains(canonical) {
                matched.push(canonical.clone());
            }
        }
    }

    matched
}

/// Word-boundary match to avoid partial matches (e.g. "requests" ≠ "request").
fn is_word_boundary_match(text: &str, word: &str) -> bool {
    let word_chars: Vec<char> = word.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();

    let mut i = 0;
    while i + word_chars.len() <= text_chars.len() {
        let slice_matches = word_chars
            .iter()
            .enumerate()
            .all(|(j, wc)| text_chars.get(i + j) == Some(wc));

        if slice_matches {
            let before_ok = i == 0 || !text_chars[i - 1].is_alphanumeric();
            let after_pos = i + word_chars.len();
            let after_ok =
                after_pos >= text_chars.len() || !text_chars[after_pos].is_alphanumeric();
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

// ─── Whitelist validation ────────────────────────────────────────────────────

/// Validate resolved packages against the whitelist.
/// Returns list of unknown packages (not in whitelist).
pub fn validate_against_whitelist(packages: &[String], language: &str) -> Vec<String> {
    let whitelist = get_whitelist();
    let known: HashSet<String> = match language {
        "python" => whitelist
            .python_packages
            .iter()
            .map(|p| p.to_lowercase())
            .collect(),
        "node" => whitelist
            .node_packages
            .iter()
            .map(|p| p.to_lowercase())
            .collect(),
        _ => HashSet::new(),
    };

    packages
        .iter()
        .filter(|p| {
            let normalized = p.to_lowercase().replace('_', "-");
            // Strip extras like [dev]
            let base = normalized.split('[').next().unwrap_or(&normalized);
            !known.contains(base)
        })
        .cloned()
        .collect()
}

// ─── Main pipeline ──────────────────────────────────────────────────────────

/// Synchronous resolution: Lock → Whitelist (no LLM).
///
/// Use this when no LLM client is available (CLI, non-agent contexts).
pub fn resolve_packages_sync(
    skill_dir: &Path,
    compatibility: Option<&str>,
    language: &str,
    allow_unknown: bool,
) -> Result<ResolvedDependencies> {
    // Layer 1: Lock file
    if let Some(packages) = resolve_from_lock(skill_dir, compatibility) {
        tracing::debug!("Resolved from lock: {:?}", packages);
        return Ok(ResolvedDependencies {
            packages,
            resolver: ResolverKind::Lock,
            unknown_packages: Vec::new(),
        });
    }

    // Layer 3: Whitelist matching (skip Layer 2 LLM — not available in sync)
    let compat_str = compatibility.unwrap_or("");
    if !compat_str.is_empty() {
        let packages = resolve_from_whitelist(compat_str, language);
        if !packages.is_empty() {
            let unknown = if allow_unknown {
                Vec::new()
            } else {
                validate_against_whitelist(&packages, language)
            };

            // Write lock file for next time
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

/// Full async resolution: Lock → LLM → Whitelist.
///
/// Only available with the `agent` feature (requires LLM client).
#[cfg(feature = "agent")]
pub async fn resolve_packages(
    skill_dir: &Path,
    compatibility: Option<&str>,
    language: &str,
    llm_client: Option<&crate::agent::llm::LlmClient>,
    model: Option<&str>,
    allow_unknown: bool,
) -> Result<ResolvedDependencies> {
    // Layer 1: Lock file
    if let Some(packages) = resolve_from_lock(skill_dir, compatibility) {
        tracing::debug!("Resolved from lock: {:?}", packages);
        return Ok(ResolvedDependencies {
            packages,
            resolver: ResolverKind::Lock,
            unknown_packages: Vec::new(),
        });
    }

    let compat_str = compatibility.unwrap_or("");

    // Layer 2: LLM inference
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

    // Layer 3: Whitelist matching
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

// ─── Embedded packages whitelist ─────────────────────────────────────────────

struct PackagesWhitelist {
    python_packages: Vec<String>,
    python_aliases: HashMap<String, String>,
    node_packages: Vec<String>,
    node_aliases: HashMap<String, String>,
}

fn get_whitelist() -> PackagesWhitelist {
    PackagesWhitelist {
        python_packages: PYTHON_PACKAGES.iter().map(|s| s.to_string()).collect(),
        python_aliases: PYTHON_ALIASES
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        node_packages: NODE_PACKAGES.iter().map(|s| s.to_string()).collect(),
        node_aliases: NODE_ALIASES
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

/// Comprehensive Python package whitelist.
/// Sync with Python SDK `packages_whitelist.json`.
const PYTHON_PACKAGES: &[&str] = &[
    // HTTP / Networking
    "requests", "httpx", "aiohttp", "urllib3", "httplib2",
    // Data Science
    "numpy", "pandas", "scipy", "scikit-learn", "statsmodels",
    // ML / AI
    "tensorflow", "keras", "torch", "pytorch", "transformers",
    "xgboost", "lightgbm", "catboost", "onnx", "onnxruntime",
    // Visualization
    "matplotlib", "seaborn", "plotly", "bokeh", "altair",
    // Web Frameworks
    "flask", "django", "fastapi", "starlette", "uvicorn", "gunicorn",
    "sanic", "tornado", "bottle", "pyramid",
    // Scraping / Parsing
    "beautifulsoup4", "lxml", "scrapy", "selenium", "playwright",
    "html5lib", "cssselect",
    // Image / Media
    "pillow", "opencv-python", "imageio", "scikit-image",
    // YAML / Config
    "pyyaml", "toml", "tomli", "python-dotenv", "configparser",
    // Database
    "sqlalchemy", "psycopg2", "psycopg2-binary", "pymysql", "redis",
    "pymongo", "motor", "asyncpg", "aiosqlite", "peewee",
    // Cloud
    "boto3", "botocore", "google-cloud-storage", "google-auth",
    "azure-storage-blob", "azure-identity",
    // Testing
    "pytest", "mock", "responses", "fakeredis", "factory-boy",
    // CLI
    "click", "typer", "argparse", "fire", "rich", "tqdm", "colorama",
    // Serialization
    "pydantic", "attrs", "dataclasses-json", "marshmallow", "cattrs",
    // Template
    "jinja2", "mako",
    // Task Queue
    "celery", "rq", "dramatiq",
    // Crypto / Auth
    "cryptography", "pyjwt", "passlib", "bcrypt", "paramiko",
    // Logging
    "loguru", "structlog",
    // Async
    "anyio", "trio",
    // Misc
    "arrow", "pendulum", "python-dateutil", "pytz",
    "chardet", "charset-normalizer",
    "tox", "nox", "pre-commit",
    "mypy", "black", "ruff", "isort",
    "setuptools", "wheel", "pip", "poetry",
];

/// Python package aliases: alias → canonical pip name.
const PYTHON_ALIASES: &[(&str, &str)] = &[
    ("cv2", "opencv-python"),
    ("PIL", "pillow"),
    ("sklearn", "scikit-learn"),
    ("bs4", "beautifulsoup4"),
    ("yaml", "pyyaml"),
    ("dotenv", "python-dotenv"),
    ("jwt", "pyjwt"),
    ("skimage", "scikit-image"),
    ("pytorch", "torch"),
    ("tf", "tensorflow"),
];

/// Comprehensive Node.js package whitelist.
const NODE_PACKAGES: &[&str] = &[
    // HTTP
    "axios", "node-fetch", "got", "superagent", "ky",
    // Web Frameworks
    "express", "koa", "fastify", "hapi", "nest", "next",
    // Utility
    "lodash", "underscore", "ramda", "fp-ts",
    // Date
    "moment", "dayjs", "date-fns", "luxon",
    // Scraping
    "cheerio", "puppeteer", "playwright",
    // Database
    "mongoose", "sequelize", "knex", "prisma", "typeorm",
    // Cache
    "ioredis", "redis",
    // Cloud
    "aws-sdk", "@aws-sdk/client-s3", "googleapis",
    // Testing
    "jest", "mocha", "chai", "vitest", "sinon",
    // CLI
    "commander", "yargs", "inquirer", "meow", "cac",
    // Output
    "chalk", "ora", "boxen", "cli-table3", "figures",
    // Config
    "dotenv", "convict",
    // Auth
    "jsonwebtoken", "bcrypt", "crypto-js", "uuid", "nanoid",
    // Realtime
    "socket.io", "ws",
    // Image
    "sharp", "jimp",
    // Frontend
    "react", "vue", "svelte", "solid-js", "angular",
    // Build
    "webpack", "vite", "esbuild", "rollup", "parcel",
    // TypeScript
    "typescript", "ts-node", "tsx",
    // Validation
    "zod", "yup", "joi", "ajv",
    // Misc
    "glob", "minimatch", "chokidar", "fs-extra",
    "debug", "winston", "pino",
    "p-limit", "p-queue", "p-retry",
    "execa", "cross-env", "cross-spawn",
    "agent-browser",
];

/// Node.js package aliases.
const NODE_ALIASES: &[(&str, &str)] = &[
    ("socket.io-client", "socket.io"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitelist_matching_python() {
        let pkgs = resolve_from_whitelist("Requires Python 3.x with requests library", "python");
        assert!(pkgs.contains(&"requests".to_string()));
    }

    #[test]
    fn test_whitelist_matching_aliases() {
        let pkgs = resolve_from_whitelist("Requires Python 3.x with cv2, PIL", "python");
        assert!(pkgs.contains(&"opencv-python".to_string()));
        assert!(pkgs.contains(&"pillow".to_string()));
    }

    #[test]
    fn test_whitelist_matching_node() {
        let pkgs = resolve_from_whitelist("Requires Node.js with axios, lodash", "node");
        assert!(pkgs.contains(&"axios".to_string()));
        assert!(pkgs.contains(&"lodash".to_string()));
    }

    #[test]
    fn test_whitelist_no_partial_match() {
        // "request" should NOT match "requests"
        let pkgs = resolve_from_whitelist("Requires request handling", "python");
        assert!(!pkgs.contains(&"requests".to_string()));
    }

    #[test]
    fn test_validate_against_whitelist() {
        let unknown = validate_against_whitelist(
            &["requests".to_string(), "my-custom-pkg".to_string()],
            "python",
        );
        assert_eq!(unknown, vec!["my-custom-pkg".to_string()]);
    }

    #[test]
    fn test_compatibility_hash_deterministic() {
        let h1 = compatibility_hash(Some("Requires Python 3.x"));
        let h2 = compatibility_hash(Some("Requires Python 3.x"));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_word_boundary_match() {
        assert!(is_word_boundary_match("requires requests library", "requests"));
        assert!(!is_word_boundary_match("requires request handling", "requests"));
        assert!(is_word_boundary_match("pandas, numpy", "pandas"));
        assert!(is_word_boundary_match("pandas, numpy", "numpy"));
    }
}
