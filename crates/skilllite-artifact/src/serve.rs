//! Run the artifact HTTP listener (shared by CLI and tests).

use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use skilllite_core::artifact_store::ArtifactStore;

use crate::Result;
use crate::{artifact_router, ArtifactHttpServerConfig, ArtifactHttpState, LocalDirArtifactStore};

/// When set to `1`, refuse to start [`run_artifact_http_server`] without a non-empty bearer token
/// (even on loopback). Defense-in-depth for shared machines or accidental exposure.
pub const ARTIFACT_HTTP_REQUIRE_AUTH_ENV: &str = "SKILLLITE_ARTIFACT_HTTP_REQUIRE_AUTH";

/// When set to `1`, allow binding on a **non-loopback** address without bearer authentication.
/// Default is **refuse** (misconfiguration guard). Strongly discouraged outside controlled lab setups.
pub const ARTIFACT_HTTP_ALLOW_INSECURE_NO_AUTH_ENV: &str =
    "SKILLLITE_ARTIFACT_HTTP_ALLOW_INSECURE_NO_AUTH";

fn normalize_bearer_token(token: Option<String>) -> Option<String> {
    token.and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    })
}

fn env_flag_true(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .as_deref()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

/// Enforce bind/auth policy before opening a socket.
///
/// - No bearer: loopback → **warn**, continue.
/// - No bearer: non-loopback → **error** unless [`ARTIFACT_HTTP_ALLOW_INSECURE_NO_AUTH_ENV`]`=1` (then **warn**).
/// - [`ARTIFACT_HTTP_REQUIRE_AUTH_ENV`]`=1`: bearer required regardless of bind address.
pub(crate) fn artifact_http_startup_policy(
    bind: SocketAddr,
    bearer_token: Option<&str>,
    require_auth: bool,
    allow_insecure_non_loopback_no_auth: bool,
) -> Result<()> {
    let has_auth = bearer_token.map(|s| !s.trim().is_empty()).unwrap_or(false);

    if require_auth && !has_auth {
        return Err(crate::Error::validation(format!(
            "refusing to start artifact HTTP: {ARTIFACT_HTTP_REQUIRE_AUTH_ENV}=1 requires a non-empty bearer token (--token or embedder-supplied)"
        )));
    }

    if has_auth {
        return Ok(());
    }

    if bind.ip().is_loopback() {
        tracing::warn!(
            target: "skilllite_artifact::http",
            %bind,
            "artifact HTTP: no Authorization bearer configured; acceptable only for trusted local tooling on loopback. \
        Set --token (or pass a token from the embedder) before exposing this service beyond localhost or shared-user hosts. \
        To hard-fail without a token even on loopback, set {ARTIFACT_HTTP_REQUIRE_AUTH_ENV}=1."
        );
        return Ok(());
    }

    if !allow_insecure_non_loopback_no_auth {
        return Err(crate::Error::validation(format!(
            "refusing to bind artifact HTTP on {bind} without a bearer token (prevents unauthenticated exposure on non-loopback addresses). \
Use loopback (e.g. 127.0.0.1:PORT), set --token, or set {ARTIFACT_HTTP_ALLOW_INSECURE_NO_AUTH_ENV}=1 to override (not recommended)."
        )));
    }

    tracing::warn!(
        target: "skilllite_artifact::http",
        %bind,
        "artifact HTTP: {ARTIFACT_HTTP_ALLOW_INSECURE_NO_AUTH_ENV}=1 — serving WITHOUT Authorization on a non-loopback address; this is insecure"
    );
    Ok(())
}

fn apply_env_artifact_http_startup_policy(
    bind: SocketAddr,
    bearer_token: Option<&str>,
) -> Result<()> {
    let require_auth = env_flag_true(ARTIFACT_HTTP_REQUIRE_AUTH_ENV);
    let allow_insecure = env_flag_true(ARTIFACT_HTTP_ALLOW_INSECURE_NO_AUTH_ENV);
    artifact_http_startup_policy(bind, bearer_token, require_auth, allow_insecure)
}

/// Bind, print `SKILLLITE_ARTIFACT_HTTP_ADDR=...` to stdout, then serve until the process exits.
pub async fn run_artifact_http_server(
    data_dir: PathBuf,
    bind: SocketAddr,
    bearer_token: Option<String>,
) -> Result<()> {
    let bearer_token = normalize_bearer_token(bearer_token);
    apply_env_artifact_http_startup_policy(bind, bearer_token.as_deref())?;

    let store: Arc<dyn ArtifactStore> = Arc::new(LocalDirArtifactStore::new(data_dir));
    let state = ArtifactHttpState::new(store, ArtifactHttpServerConfig { bearer_token });
    let app = artifact_router(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let local = listener.local_addr()?;
    println!("SKILLLITE_ARTIFACT_HTTP_ADDR={}", local);
    std::io::stdout().flush()?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod startup_policy_tests {
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

    use super::artifact_http_startup_policy;

    fn lo(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port))
    }

    fn any_iface(port: u16) -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port))
    }

    #[test]
    fn allows_non_loopback_when_token_set() {
        artifact_http_startup_policy(any_iface(8080), Some("secret"), false, false).unwrap();
    }

    #[test]
    fn allows_loopback_without_token_when_not_required() {
        artifact_http_startup_policy(lo(8080), None, false, false).unwrap();
    }

    #[test]
    fn rejects_loopback_without_token_when_require_auth() {
        let err = artifact_http_startup_policy(lo(8080), None, true, false).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("REQUIRE_AUTH") || msg.contains("bearer token"),
            "unexpected message: {msg}"
        );
    }

    #[test]
    fn rejects_non_loopback_without_token_by_default() {
        let err = artifact_http_startup_policy(any_iface(8080), None, false, false).unwrap_err();
        assert!(err.to_string().contains("refusing to bind"));
    }

    #[test]
    fn allows_non_loopback_without_token_when_insecure_override() {
        artifact_http_startup_policy(any_iface(8080), None, false, true).unwrap();
    }

    #[test]
    fn whitespace_only_token_counts_as_missing_for_require_auth() {
        let err = artifact_http_startup_policy(lo(9), Some(" \t "), true, false).unwrap_err();
        assert!(err.to_string().contains("bearer token"));
    }
}
