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
//!
//! Three-layer resolution for skill dependencies. Used by `skilllite init` when
//! .skilllite.lock is missing or stale.

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
    /// Packages not found in the whitelist (non-empty only when allow_unknown=false).
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
    "html5lib", "cssselect", "html2text",
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
