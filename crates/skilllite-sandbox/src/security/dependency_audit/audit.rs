//! Public entry point and dependency collection.

use std::collections::HashSet;
use std::path::Path;

use crate::Result;

use super::super::malicious_packages::{check_malicious_packages, MaliciousPackageHit};
use super::backends::{query_osv_batch, query_pypi};
use super::config::{self, get_custom_api, get_osv_api_base, get_pypi_base, make_agent};
use super::parsers::{parse_lock_file, parse_package_json, parse_requirements_txt};
use super::resolve::resolve_from_metadata_fields;
use super::types::{
    AuditBackend, Dependency, DependencyAuditResult, MetadataHint, PackageAuditEntry,
};

/// Collect all dependencies from a skill directory.
pub(super) fn collect_dependencies(
    skill_dir: &Path,
    metadata_hint: Option<&MetadataHint>,
) -> Vec<Dependency> {
    let mut deps = Vec::new();
    let mut seen = HashSet::new();

    let req_txt = skill_dir.join("requirements.txt");
    if req_txt.exists() {
        if let Ok(content) = std::fs::read_to_string(&req_txt) {
            for dep in parse_requirements_txt(&content) {
                seen.insert((dep.name.to_lowercase(), dep.ecosystem.clone()));
                deps.push(dep);
            }
        }
    }

    let pkg_json = skill_dir.join("package.json");
    if pkg_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_json) {
            for dep in parse_package_json(&content) {
                seen.insert((dep.name.to_lowercase(), dep.ecosystem.clone()));
                deps.push(dep);
            }
        }
    }

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

/// Run a full dependency audit on a skill directory.
pub fn audit_skill_dependencies(
    skill_dir: &Path,
    metadata_hint: Option<&MetadataHint>,
) -> Result<DependencyAuditResult> {
    let deps = collect_dependencies(skill_dir, metadata_hint);

    let malicious_hits =
        check_malicious_packages(deps.iter().map(|d| (d.name.as_str(), d.ecosystem.as_str())));
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

    if let Some(custom_url) = get_custom_api() {
        tracing::info!(
            "Scanning {} dependencies via custom API ({})...",
            deps.len(),
            custom_url
        );
        let entries = query_osv_batch(&agent, &deps, &custom_url)?;
        return Ok(build_result(
            entries,
            AuditBackend::Custom(custom_url),
            malicious_hits,
        ));
    }

    let pypi_deps: Vec<_> = deps
        .iter()
        .filter(|d| d.ecosystem == "PyPI")
        .cloned()
        .collect();
    let npm_deps: Vec<_> = deps
        .iter()
        .filter(|d| d.ecosystem == "npm")
        .cloned()
        .collect();

    let mut all_entries = Vec::new();

    if !pypi_deps.is_empty() {
        let pypi_base = get_pypi_base();
        let mirror_note = if pypi_base != config::DEFAULT_PYPI_BASE {
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

    if !npm_deps.is_empty() {
        let osv_base = get_osv_api_base();
        let mirror_note = if osv_base != config::DEFAULT_OSV_API_BASE {
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

    Ok(build_result(
        all_entries,
        AuditBackend::Native,
        malicious_hits,
    ))
}

pub(crate) fn build_result(
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
