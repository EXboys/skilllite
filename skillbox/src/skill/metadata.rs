use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::Path;

/// Front matter data (official Agent Skills fields per Claude specification)
/// See: https://docs.anthropic.com/en/docs/agents-and-tools/agent-skills/specification
#[derive(Deserialize, Debug, Clone, Default)]
#[allow(dead_code)]
struct FrontMatter {
    /// Required: Skill name (max 64 chars, lowercase + hyphens only)
    #[serde(default)]
    pub name: String,

    /// Required: Description of what the skill does (max 1024 chars)
    #[serde(default)]
    pub description: Option<String>,

    /// Optional: License name or reference
    #[serde(default)]
    pub license: Option<String>,

    /// Optional: Environment requirements (max 500 chars)
    /// Examples: "Requires Python 3.x, network access", "Requires git, docker"
    #[serde(default)]
    pub compatibility: Option<String>,

    /// Optional: Additional metadata (author, version, etc.)
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,

    /// Optional: Pre-approved tools (experimental)
    #[serde(default, rename = "allowed-tools")]
    pub allowed_tools: Option<String>,
}

/// Skill metadata parsed from SKILL.md YAML front matter
#[derive(Debug, Clone)]
pub struct SkillMetadata {
    /// Skill name
    pub name: String,

    /// Entry point script path (relative to skill directory)
    pub entry_point: String,

    /// Programming language: "python", "node", or "bash"
    pub language: Option<String>,

    /// Description of the skill
    pub description: Option<String>,

    /// Compatibility string (environment requirements)
    pub compatibility: Option<String>,

    /// Network policy configuration (derived from compatibility)
    pub network: NetworkPolicy,

    /// Resolved package list from .skilllite.lock (written by `skilllite init`).
    /// When present, this takes priority over parsing the compatibility field.
    pub resolved_packages: Option<Vec<String>>,
}

impl SkillMetadata {
    /// Returns true if this skill depends on Playwright (requires spawn/subprocess, blocked in sandbox).
    pub fn uses_playwright(&self) -> bool {
        if let Some(ref packages) = self.resolved_packages {
            if packages
                .iter()
                .any(|p| p.to_lowercase().trim() == "playwright")
            {
                return true;
            }
        }
        if let Some(ref compat) = self.compatibility {
            if compat.to_lowercase().contains("playwright") {
                return true;
            }
        }
        false
    }
}

/// Network access policy (derived from compatibility field)
#[derive(Debug, Clone, Default)]
pub struct NetworkPolicy {
    /// Whether network access is enabled
    pub enabled: bool,

    /// List of allowed outbound hosts (e.g., ["*:80", "*:443"])
    /// When network is enabled via compatibility, defaults to allow all HTTP/HTTPS
    pub outbound: Vec<String>,
}

/// Parse compatibility string to extract network policy
/// Examples:
///   - "Requires network access" -> enabled=true
///   - "Requires Python 3.x, internet" -> enabled=true
///   - "Requires git, docker" -> enabled=false
fn parse_compatibility_for_network(compatibility: Option<&str>) -> NetworkPolicy {
    let Some(compat) = compatibility else {
        return NetworkPolicy::default();
    };

    let compat_lower = compat.to_lowercase();
    
    // Check for network/internet keywords
    let needs_network = compat_lower.contains("network")
        || compat_lower.contains("internet")
        || compat_lower.contains("http")
        || compat_lower.contains("api")
        || compat_lower.contains("web");

    if needs_network {
        NetworkPolicy {
            enabled: true,
            // Allow all domains by default when network is enabled via compatibility
            // The "*" wildcard matches all domains in ProxyConfig::domain_matches
            outbound: vec!["*".to_string()],
        }
    } else {
        NetworkPolicy::default()
    }
}

/// Parse compatibility string to detect language
/// Examples:
///   - "Requires Python 3.x" -> Some("python")
///   - "Requires Node.js" -> Some("node")
///   - "Requires bash" -> Some("bash")
fn parse_compatibility_for_language(compatibility: Option<&str>) -> Option<String> {
    let compat = compatibility?;
    let compat_lower = compat.to_lowercase();

    if compat_lower.contains("python") {
        Some("python".to_string())
    } else if compat_lower.contains("node") || compat_lower.contains("javascript") || compat_lower.contains("typescript") {
        Some("node".to_string())
    } else if compat_lower.contains("bash") || compat_lower.contains("shell") {
        Some("bash".to_string())
    } else {
        None
    }
}

/// Auto-detect entry point from skill directory.
/// Looks for main.{py,js,ts,sh} in scripts/ directory.
fn detect_entry_point(skill_dir: &Path) -> Option<String> {
    let scripts_dir = skill_dir.join("scripts");
    if !scripts_dir.exists() {
        return None;
    }

    // Check for main files in priority order
    for ext in [".py", ".js", ".ts", ".sh"] {
        let main_file = scripts_dir.join(format!("main{}", ext));
        if main_file.exists() {
            return Some(format!("scripts/main{}", ext));
        }
    }

    // Check for index files (common in Node.js)
    for ext in [".py", ".js", ".ts", ".sh"] {
        let index_file = scripts_dir.join(format!("index{}", ext));
        if index_file.exists() {
            return Some(format!("scripts/index{}", ext));
        }
    }

    // If only one script file exists, use it
    let mut script_files = Vec::new();
    if let Ok(entries) = fs::read_dir(&scripts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy();
                if ["py", "js", "ts", "sh"].contains(&ext_str.as_ref()) {
                    // Skip test files and __init__.py
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if !name.starts_with("test_") 
                        && !name.ends_with("_test.py")
                        && name != "__init__.py"
                        && !name.starts_with('.') {
                        script_files.push(format!("scripts/{}", name));
                    }
                }
            }
        }
    }

    if script_files.len() == 1 {
        return Some(script_files.remove(0));
    }

    None
}

/// Auto-detect language from entry point extension
fn detect_language_from_entry_point(entry_point: &str) -> Option<String> {
    if entry_point.ends_with(".py") {
        Some("python".to_string())
    } else if entry_point.ends_with(".js") || entry_point.ends_with(".ts") {
        Some("node".to_string())
    } else if entry_point.ends_with(".sh") {
        Some("bash".to_string())
    } else {
        None
    }
}

/// Parse SKILL.md file and extract metadata from YAML front matter
pub fn parse_skill_metadata(skill_dir: &Path) -> Result<SkillMetadata> {
    let skill_md_path = skill_dir.join("SKILL.md");

    if !skill_md_path.exists() {
        anyhow::bail!(
            "SKILL.md not found in directory: {}",
            skill_dir.display()
        );
    }

    let content = fs::read_to_string(&skill_md_path)
        .with_context(|| format!("Failed to read SKILL.md: {}", skill_md_path.display()))?;

    extract_yaml_front_matter_with_detection(&content, skill_dir)
}

/// Extract YAML front matter from markdown content (for tests without skill_dir)
#[cfg(test)]
fn extract_yaml_front_matter(content: &str) -> Result<SkillMetadata> {
    extract_yaml_front_matter_impl(content, None)
}

/// Extract YAML front matter from markdown content with auto-detection
fn extract_yaml_front_matter_with_detection(content: &str, skill_dir: &Path) -> Result<SkillMetadata> {
    extract_yaml_front_matter_impl(content, Some(skill_dir))
}

/// Extract YAML front matter from markdown content
fn extract_yaml_front_matter_impl(content: &str, skill_dir: Option<&Path>) -> Result<SkillMetadata> {
    // Match YAML front matter between --- delimiters
    let re = Regex::new(r"(?s)^---\s*\n(.*?)\n---")
        .expect("SKILL.md front matter regex is valid");

    let captures = re
        .captures(content)
        .ok_or_else(|| anyhow::anyhow!("No YAML front matter found in SKILL.md"))?;

    let yaml_content = captures
        .get(1)
        .ok_or_else(|| anyhow::anyhow!("Failed to extract YAML content"))?
        .as_str();

    let front_matter: FrontMatter = serde_yaml::from_str(yaml_content)
        .with_context(|| "Failed to parse YAML front matter")?;

    // Auto-detect entry_point from scripts/ directory
    let mut entry_point = String::new();
    if let Some(dir) = skill_dir {
        if let Some(detected) = detect_entry_point(dir) {
            entry_point = detected;
        }
    }

    // Detect language: first from compatibility, then from entry_point
    let language = parse_compatibility_for_language(front_matter.compatibility.as_deref())
        .or_else(|| detect_language_from_entry_point(&entry_point));

    // Parse network policy from compatibility field
    let network = parse_compatibility_for_network(front_matter.compatibility.as_deref());

    // Read resolved_packages from .skilllite.lock (written by `skilllite init`)
    let resolved_packages = skill_dir.and_then(|dir| {
        read_lock_file_packages(dir, front_matter.compatibility.as_deref())
    });

    let metadata = SkillMetadata {
        name: front_matter.name.clone(),
        entry_point,
        language,
        description: front_matter.description.clone(),
        compatibility: front_matter.compatibility.clone(),
        network,
        resolved_packages,
    };

    // Validate required fields
    if metadata.name.is_empty() {
        anyhow::bail!("Skill name is required in SKILL.md");
    }

    Ok(metadata)
}

/// Read resolved packages from ``.skilllite.lock`` in *skill_dir*.
///
/// Returns ``None`` if the lock file is missing, invalid, or stale
/// (i.e. its ``compatibility_hash`` does not match the current compatibility string).
fn read_lock_file_packages(skill_dir: &Path, compatibility: Option<&str>) -> Option<Vec<String>> {
    let lock_path = skill_dir.join(".skilllite.lock");
    let content = fs::read_to_string(&lock_path).ok()?;
    let lock: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Staleness check: compare compatibility hash
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(compatibility.unwrap_or("").as_bytes());
    let current_hash = hex::encode(hasher.finalize());

    if lock.get("compatibility_hash")?.as_str()? != current_hash {
        return None; // stale lock
    }

    let arr = lock.get("resolved_packages")?.as_array()?;
    let packages: Vec<String> = arr
        .iter()
        .filter_map(|v| v.as_str().map(String::from))
        .collect();

    if packages.is_empty() {
        None
    } else {
        Some(packages)
    }
}

/// Detect language from skill directory if not specified
/// Language is detected from:
/// 1. metadata.language (from compatibility field)
/// 2. Entry point file extension
/// 3. Scripts in scripts/ directory
pub fn detect_language(skill_dir: &Path, metadata: &SkillMetadata) -> String {
    // First check metadata (derived from compatibility field)
    if let Some(ref lang) = metadata.language {
        return lang.clone();
    }

    // Detect from entry point extension
    if metadata.entry_point.ends_with(".py") {
        return "python".to_string();
    }

    if metadata.entry_point.ends_with(".js") || metadata.entry_point.ends_with(".ts") {
        return "node".to_string();
    }

    if metadata.entry_point.ends_with(".sh") {
        return "bash".to_string();
    }

    // Scan scripts directory for language hints
    let scripts_dir = skill_dir.join("scripts");
    if scripts_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
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

    // Default to python
    "python".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml_front_matter_with_compatibility() {
        let content = r#"---
name: test-skill
description: A test skill for testing
compatibility: Requires Python 3.x with requests library, network access
---

# Test Skill

This is a test skill.
"#;

        let metadata = extract_yaml_front_matter(content)
            .expect("test YAML parsing should succeed");
        assert_eq!(metadata.name, "test-skill");
        assert_eq!(metadata.language, Some("python".to_string()));
        assert!(metadata.network.enabled);
        // When network is enabled via compatibility, allow all domains with "*" wildcard
        assert_eq!(metadata.network.outbound, vec!["*"]);
    }

    #[test]
    fn test_parse_compatibility_for_network() {
        // Network enabled cases
        assert!(parse_compatibility_for_network(Some("Requires network access")).enabled);
        assert!(parse_compatibility_for_network(Some("Requires internet")).enabled);
        assert!(parse_compatibility_for_network(Some("Requires http client")).enabled);
        assert!(parse_compatibility_for_network(Some("Requires API access")).enabled);
        assert!(parse_compatibility_for_network(Some("Requires web access")).enabled);

        // Network disabled cases
        assert!(!parse_compatibility_for_network(Some("Requires git, docker")).enabled);
        assert!(!parse_compatibility_for_network(Some("Requires Python 3.x")).enabled);
        assert!(!parse_compatibility_for_network(None).enabled);
    }

    #[test]
    fn test_parse_compatibility_for_language() {
        assert_eq!(parse_compatibility_for_language(Some("Requires Python 3.x")), Some("python".to_string()));
        assert_eq!(parse_compatibility_for_language(Some("Requires Node.js")), Some("node".to_string()));
        assert_eq!(parse_compatibility_for_language(Some("Requires JavaScript")), Some("node".to_string()));
        assert_eq!(parse_compatibility_for_language(Some("Requires bash")), Some("bash".to_string()));
        assert_eq!(parse_compatibility_for_language(Some("Requires git, docker")), None);
        assert_eq!(parse_compatibility_for_language(None), None);
    }

    #[test]
    fn test_default_network_policy() {
        let content = r#"---
name: simple-skill
description: A simple skill
---
"#;

        let metadata = extract_yaml_front_matter(content)
            .expect("test YAML parsing should succeed");
        assert!(!metadata.network.enabled);
        assert!(metadata.network.outbound.is_empty());
    }
}
