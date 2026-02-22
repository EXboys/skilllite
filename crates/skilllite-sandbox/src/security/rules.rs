//! Security rule definitions and configuration
//!
//! This module provides the `SecurityRule` struct for defining security scanning rules
//! and `RulesConfig` for loading custom rules from configuration files.

#![allow(dead_code)]

use super::types::{SecurityIssueType, SecuritySeverity};
use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// A single security scanning rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityRule {
    /// Unique identifier for the rule
    pub id: String,
    /// Regular expression pattern to match
    pub pattern: String,
    /// Type of security issue this rule detects
    pub issue_type: SecurityIssueType,
    /// Severity level of the issue
    pub severity: SecuritySeverity,
    /// Human-readable description of the issue
    pub description: String,
    /// Languages this rule applies to (e.g., ["python", "javascript"])
    #[serde(default)]
    pub languages: Vec<String>,
    /// Whether this rule is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl SecurityRule {
    /// Create a new security rule
    pub fn new(
        id: &str,
        pattern: &str,
        issue_type: SecurityIssueType,
        severity: SecuritySeverity,
        description: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            pattern: pattern.to_string(),
            issue_type,
            severity,
            description: description.to_string(),
            languages: Vec::new(),
            enabled: true,
        }
    }

    /// Set the languages this rule applies to
    pub fn for_languages(mut self, languages: &[&str]) -> Self {
        self.languages = languages.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Compile the regex pattern
    pub fn compile(&self) -> Result<Regex> {
        Regex::new(&self.pattern).with_context(|| {
            format!(
                "Failed to compile regex for rule '{}': {}",
                self.id, self.pattern
            )
        })
    }
}

/// Configuration for security rules
///
/// This struct can be loaded from a YAML configuration file to customize
/// the security scanning behavior.
///
/// # Example YAML Configuration
///
/// ```yaml
/// # .skilllite-rules.yaml
/// use_default_rules: true
/// disabled_rules:
///   - py-file-open  # Disable the open() detection rule
/// rules:
///   - id: custom-dangerous-func
///     pattern: "dangerous_function\\s*\\("
///     issue_type: code_injection
///     severity: high
///     description: "Custom dangerous function detected"
///     languages: ["python"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RulesConfig {
    /// Custom rules to add
    #[serde(default)]
    pub rules: Vec<SecurityRule>,
    /// Rule IDs to disable from the default set
    #[serde(default)]
    pub disabled_rules: Vec<String>,
    /// Whether to use default rules (default: true)
    #[serde(default = "default_use_defaults")]
    pub use_default_rules: bool,
}

fn default_use_defaults() -> bool {
    true
}

impl RulesConfig {
    /// Load rules configuration from a YAML file
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read rules config: {}", path.display()))?;
        serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse rules config: {}", path.display()))
    }

    /// Try to load rules from the skill directory or use defaults
    ///
    /// Looks for configuration files in the following order:
    /// 1. `.skilllite-rules.yaml`
    /// 2. `.skilllite-rules.yml`
    /// 3. `skilllite-rules.yaml`
    /// 4. `.skillbox-rules.yaml` (legacy, backward compat)
    /// 5. `.skillbox-rules.yml`
    /// 6. `skillbox-rules.yaml`
    pub fn load_or_default(skill_dir: Option<&Path>) -> Self {
        if let Some(dir) = skill_dir {
            for name in [
                ".skilllite-rules.yaml",
                ".skilllite-rules.yml",
                "skilllite-rules.yaml",
                ".skillbox-rules.yaml",
                ".skillbox-rules.yml",
                "skillbox-rules.yaml",
            ] {
                let config_path = dir.join(name);
                if config_path.exists() {
                    if let Ok(config) = Self::load_from_file(&config_path) {
                        return config;
                    }
                }
            }
        }
        Self::default()
    }
}

/// Configuration file names that are recognized (primary: skilllite, legacy: skillbox)
pub const CONFIG_FILE_NAMES: &[&str] = &[
    ".skilllite-rules.yaml",
    ".skilllite-rules.yml",
    "skilllite-rules.yaml",
    ".skillbox-rules.yaml",
    ".skillbox-rules.yml",
    "skillbox-rules.yaml",
];
