//! Types for dependency audit.

use serde::{Deserialize, Serialize};

use super::super::malicious_packages::MaliciousPackageHit;

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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub malicious: Vec<MaliciousPackageHit>,
}

/// Metadata hint for dependency inference when no explicit dependency files exist.
#[derive(Debug, Clone)]
pub struct MetadataHint {
    pub compatibility: Option<String>,
    pub resolved_packages: Option<Vec<String>>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub entry_point: String,
}
