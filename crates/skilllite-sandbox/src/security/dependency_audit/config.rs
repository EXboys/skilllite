//! Configuration and HTTP agent for audit backends.

pub(super) const DEFAULT_PYPI_BASE: &str = "https://pypi.org";
pub(super) const DEFAULT_OSV_API_BASE: &str = "https://api.osv.dev";

/// Get custom audit API URL, if configured.
pub(super) fn get_custom_api() -> Option<String> {
    skilllite_core::config::load_dotenv();
    skilllite_core::config::env_optional(
        skilllite_core::config::env_keys::misc::SKILLLITE_AUDIT_API,
        &[],
    )
    .map(|s| s.trim_end_matches('/').to_string())
}

/// Get PyPI mirror base URL.
pub(super) fn get_pypi_base() -> String {
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
pub(super) fn get_osv_api_base() -> String {
    skilllite_core::config::load_dotenv();
    skilllite_core::config::env_or(
        skilllite_core::config::env_keys::misc::OSV_API_URL,
        &[],
        || DEFAULT_OSV_API_BASE.to_string(),
    )
    .trim_end_matches('/')
    .to_string()
}

pub(super) fn make_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(std::time::Duration::from_secs(10))
        .timeout_read(std::time::Duration::from_secs(30))
        .build()
}
