//! Supply chain vulnerability scanning — multi-backend architecture.
//!
//! Parses dependency files (requirements.txt, package.json) from skill directories
//! and queries vulnerability databases for known issues.
//!
//! # Backend priority
//!
//! 1. **Custom API** (`SKILLLITE_AUDIT_API`): Your own security service endpoint.
//! 2. **PyPI JSON API** (Python packages only): Queries PyPI directly.
//! 3. **OSV.dev API** (npm, fallback): Batch query against Google's OSV database.
//!
//! # Environment variables
//!
//! | Variable | Default | Description |
//! |----------|---------|-------------|
//! | `SKILLLITE_AUDIT_API` | *(none)* | Custom security API (overrides all other backends) |
//! | `PYPI_MIRROR_URL` | `https://pypi.org` | PyPI mirror for Python vulnerability queries |
//! | `OSV_API_URL` | `https://api.osv.dev` | OSV API for npm / fallback queries |

mod audit;
mod backends;
mod config;
mod format;
mod parsers;
mod resolve;
mod types;

#[cfg(test)]
mod tests;

pub use audit::audit_skill_dependencies;
pub use format::{format_audit_result, format_audit_result_json};
pub use types::{
    AuditBackend, Dependency, DependencyAuditResult, MetadataHint, PackageAuditEntry, VulnRef,
};
