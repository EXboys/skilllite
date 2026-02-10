use crate::skill::metadata::SkillMetadata;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Dependency information derived from compatibility field
#[derive(Debug, Clone)]
pub struct DependencyInfo {
    /// Type of dependency
    pub dep_type: DependencyType,
    /// List of packages extracted from compatibility
    pub packages: Vec<String>,
    /// SHA256 hash of the packages list
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DependencyType {
    /// Python packages
    Python,
    /// Node.js packages
    Node,
    /// No dependencies
    None,
}

/// Detect dependencies from compatibility field in metadata
/// 
/// The compatibility field follows Claude Agent Skills specification:
/// Examples:
///   - "Requires Python 3.x with requests library"
///   - "Requires Python 3.x, pandas, numpy"
///   - "Requires Node.js with axios"
pub fn detect_dependencies(_skill_dir: &Path, metadata: &SkillMetadata) -> Result<DependencyInfo> {
    let language = crate::skill::metadata::detect_language(_skill_dir, metadata);

    // Priority 1: Use resolved_packages from .skilllite.lock if available
    // Priority 2: Fallback to parsing compatibility field with hardcoded whitelist
    let packages = if let Some(ref resolved) = metadata.resolved_packages {
        resolved.clone()
    } else {
        parse_compatibility_for_packages(metadata.compatibility.as_deref())
    };

    if packages.is_empty() {
        return Ok(DependencyInfo {
            dep_type: DependencyType::None,
            packages: vec![],
            content_hash: String::new(),
        });
    }

    let hash = compute_packages_hash(&packages);
    let dep_type = match language.as_str() {
        "python" => DependencyType::Python,
        "node" => DependencyType::Node,
        _ => DependencyType::None,
    };

    Ok(DependencyInfo {
        dep_type,
        packages,
        content_hash: hash,
    })
}

/// Parse compatibility string to extract package names
/// 
/// Examples:
///   - "Requires Python 3.x with requests library" -> ["requests"]
///   - "Requires Python 3.x, pandas, numpy, network access" -> ["pandas", "numpy"]
///   - "Requires Node.js with axios, lodash" -> ["axios", "lodash"]
fn parse_compatibility_for_packages(compatibility: Option<&str>) -> Vec<String> {
    let Some(compat) = compatibility else {
        return vec![];
    };

    // Common Python packages that might appear in compatibility
    let known_python_packages = [
        "requests", "pandas", "numpy", "scipy", "matplotlib", "seaborn",
        "sklearn", "scikit-learn", "tensorflow", "torch", "pytorch",
        "flask", "django", "fastapi", "aiohttp", "httpx",
        "beautifulsoup", "bs4", "lxml", "selenium",
        "pillow", "opencv", "cv2", "pyyaml", "yaml",
        "sqlalchemy", "psycopg2", "pymysql", "redis", "pymongo",
        "boto3", "google-cloud", "azure",
        "pytest", "unittest", "mock",
        "click", "argparse", "typer",
        "pydantic", "dataclasses", "attrs",
        "jinja2", "mako",
        "celery", "rq",
        "cryptography", "jwt", "passlib",
        "playwright",
    ];

    // Common Node.js packages (excluding 'request' which conflicts with Python 'requests')
    let known_node_packages = [
        "axios", "node-fetch", "got",
        "express", "koa", "fastify", "hapi",
        "lodash", "underscore", "ramda",
        "moment", "dayjs", "date-fns",
        "cheerio", "puppeteer", "playwright",
        "mongoose", "sequelize", "knex", "prisma",
        "ioredis",
        "aws-sdk", "googleapis",
        "jest", "mocha", "chai",
        "commander", "yargs", "inquirer",
        "chalk", "ora", "boxen",
        "dotenv",
        "jsonwebtoken", "bcrypt", "crypto-js",
        "socket.io", "ws",
        "sharp", "jimp",
    ];

    let compat_lower = compat.to_lowercase();
    let mut packages = Vec::new();

    // Check for known Python packages using word boundary matching
    for pkg in known_python_packages.iter() {
        let pkg_lower = pkg.to_lowercase();
        // Use word boundary matching to avoid partial matches
        // e.g., "requests" should not match "request"
        if is_word_match(&compat_lower, &pkg_lower) {
            packages.push(pkg.to_string());
        }
    }

    // Check for known Node.js packages using word boundary matching
    for pkg in known_node_packages.iter() {
        let pkg_lower = pkg.to_lowercase();
        if is_word_match(&compat_lower, &pkg_lower) {
            packages.push(pkg.to_string());
        }
    }

    packages
}

/// Check if a word appears as a complete word in the text
/// This prevents "requests" from matching "request"
fn is_word_match(text: &str, word: &str) -> bool {
    // Simple word boundary check
    let word_chars: Vec<char> = word.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    
    let mut i = 0;
    while i <= text_chars.len().saturating_sub(word_chars.len()) {
        // Check if word matches at position i
        let mut matches = true;
        for (j, wc) in word_chars.iter().enumerate() {
            if text_chars.get(i + j) != Some(wc) {
                matches = false;
                break;
            }
        }
        
        if matches {
            // Check word boundaries
            let before_ok = i == 0 || !text_chars[i - 1].is_alphanumeric();
            let after_pos = i + word_chars.len();
            let after_ok = after_pos >= text_chars.len() || !text_chars[after_pos].is_alphanumeric();
            
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// Compute hash from package list.
///
/// The list is **sorted** before hashing so that different orderings of the
/// same packages always produce the same hash.
fn compute_packages_hash(packages: &[String]) -> String {
    let mut sorted_packages: Vec<&String> = packages.iter().collect();
    sorted_packages.sort();

    let mut hasher = Sha256::new();
    for pkg in sorted_packages {
        hasher.update(pkg.as_bytes());
        hasher.update(b"\n");
    }
    let result = hasher.finalize();
    hex::encode(result)
}

/// Validate dependencies (now just validates compatibility field format)
pub fn validate_dependencies(_skill_dir: &Path, _metadata: &SkillMetadata) -> Result<()> {
    // No validation needed - compatibility is a free-form string
    // per Claude Agent Skills specification
    Ok(())
}

/// Get the cache key for a dependency configuration
pub fn get_cache_key(dep_info: &DependencyInfo) -> String {
    match dep_info.dep_type {
        DependencyType::Python => {
            if dep_info.content_hash.is_empty() {
                "py-none".to_string()
            } else {
                format!("py-{}", &dep_info.content_hash[..16.min(dep_info.content_hash.len())])
            }
        }
        DependencyType::Node => {
            if dep_info.content_hash.is_empty() {
                "node-none".to_string()
            } else {
                format!("node-{}", &dep_info.content_hash[..16.min(dep_info.content_hash.len())])
            }
        }
        DependencyType::None => "none".to_string(),
    }
}
