//! OSV and PyPI audit backends.

use anyhow::{Context, Result};
use serde::Deserialize;

use super::types::{Dependency, PackageAuditEntry, VulnRef};

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
pub(super) fn query_osv_batch(
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

        let response = agent.post(&batch_url).send_json(&body).map_err(|e| match &e {
            ureq::Error::Status(code, _) => anyhow::anyhow!("Audit API returned HTTP {} — {}", code, e),
            ureq::Error::Transport(_) => {
                anyhow::anyhow!("Cannot reach audit API at {} : {}", api_base, e)
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
pub(super) fn query_pypi(
    agent: &ureq::Agent,
    deps: &[Dependency],
    pypi_base: &str,
) -> Result<Vec<PackageAuditEntry>> {
    let mut entries = Vec::new();

    for dep in deps {
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
                    let pypi: PypiResponse = response
                        .into_json()
                        .unwrap_or(PypiResponse {
                            vulnerabilities: Vec::new(),
                        });

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
                    let pypi: PypiInfoResponse = response.into_json().unwrap_or(PypiInfoResponse {
                        info: PypiInfo {
                            version: "latest".to_string(),
                        },
                        vulnerabilities: Vec::new(),
                    });

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
                    dep.name,
                    version_display,
                    e
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
