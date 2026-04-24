//! `skilllite gateway serve` — unified HTTP host for health, inbound webhook, and optional artifacts.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use skilllite_artifact::{
    artifact_router, ArtifactHttpServerConfig, ArtifactHttpState, LocalDirArtifactStore,
};
use skilllite_commands::channel_serve::channel_webhook_router;
use skilllite_core::artifact_store::ArtifactStore;
use tower_http::trace::TraceLayer;

use crate::cli::{Commands, GatewayAction};
use crate::command_registry::CommandRegistry;
use crate::Error;

/// Env var that must equal `1` before `gateway serve` will bind (CLI only).
pub const GATEWAY_SERVE_ALLOW_ENV: &str = "SKILLLITE_GATEWAY_SERVE_ALLOW";

/// When set to `1`, allow binding on a non-loopback address without bearer authentication.
pub const GATEWAY_HTTP_ALLOW_INSECURE_NO_AUTH_ENV: &str =
    "SKILLLITE_GATEWAY_HTTP_ALLOW_INSECURE_NO_AUTH";

pub fn register(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::Gateway { action } = cmd {
            match action {
                GatewayAction::Serve {
                    bind,
                    token,
                    artifact_dir,
                } => Some(run_gateway_serve(
                    bind.clone(),
                    token.clone(),
                    artifact_dir.clone(),
                )),
            }
        } else {
            None
        }
    });
}

fn run_gateway_serve(
    bind: String,
    token: Option<String>,
    artifact_dir: Option<PathBuf>,
) -> Result<(), Error> {
    let allowed = std::env::var(GATEWAY_SERVE_ALLOW_ENV)
        .map(|v| v.trim() == "1")
        .unwrap_or(false);
    if !allowed {
        return Err(Error::msg(format!(
            "refusing to start gateway HTTP server: set {GATEWAY_SERVE_ALLOW_ENV}=1 to bind (avoids accidental network exposure)"
        )));
    }

    let addr: SocketAddr = bind
        .parse()
        .map_err(|e| Error::msg(format!("invalid --bind {bind:?}: {e}")))?;
    let token = normalize_token(token);
    gateway_http_startup_policy(
        addr,
        token.as_deref(),
        env_flag_true(GATEWAY_HTTP_ALLOW_INSECURE_NO_AUTH_ENV),
    )?;

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::msg(format!("tokio runtime: {e}")))?;
    rt.block_on(run_gateway_http_server(addr, token, artifact_dir))
}

async fn run_gateway_http_server(
    bind: SocketAddr,
    token: Option<String>,
    artifact_dir: Option<PathBuf>,
) -> Result<(), Error> {
    let app = build_gateway_app(token.clone(), artifact_dir);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    let local = listener.local_addr()?;
    eprintln!("SKILLLITE_GATEWAY_HTTP_ADDR={local}");
    eprintln!(
        "gateway serve: GET /health  POST /webhook/inbound{}  (set {}=1 to allow bind)",
        if token.is_some() {
            "  Authorization: Bearer required"
        } else {
            ""
        },
        GATEWAY_SERVE_ALLOW_ENV
    );
    axum::serve(listener, app)
        .await
        .map_err(|e| Error::msg(format!("server: {e}")))
}

fn build_gateway_app(token: Option<String>, artifact_dir: Option<PathBuf>) -> Router {
    let mut app = Router::new()
        .route("/health", get(health))
        .merge(channel_webhook_router(token.clone()));

    if let Some(dir) = artifact_dir {
        let store: Arc<dyn ArtifactStore> = Arc::new(LocalDirArtifactStore::new(dir));
        let state = ArtifactHttpState::new(
            store,
            ArtifactHttpServerConfig {
                bearer_token: token.clone(),
            },
        );
        app = app.merge(artifact_router(state));
    }

    app.layer(TraceLayer::new_for_http())
}

fn normalize_token(token: Option<String>) -> Option<String> {
    token.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
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

fn gateway_http_startup_policy(
    bind: SocketAddr,
    bearer_token: Option<&str>,
    allow_insecure_non_loopback_no_auth: bool,
) -> Result<(), Error> {
    let has_auth = bearer_token.map(|s| !s.trim().is_empty()).unwrap_or(false);
    if has_auth || bind.ip().is_loopback() {
        return Ok(());
    }
    if allow_insecure_non_loopback_no_auth {
        tracing::warn!(
            %bind,
            "gateway HTTP: {GATEWAY_HTTP_ALLOW_INSECURE_NO_AUTH_ENV}=1 — serving without Authorization on a non-loopback address; this is insecure"
        );
        return Ok(());
    }
    Err(Error::msg(format!(
        "non-loopback --bind requires --token or set {GATEWAY_HTTP_ALLOW_INSECURE_NO_AUTH_ENV}=1 (unsafe)"
    )))
}

async fn health() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "service": "skilllite-gateway",
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{header, Request};
    use tempfile::TempDir;
    use tower::ServiceExt;

    fn lo(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    fn any_iface(port: u16) -> SocketAddr {
        SocketAddr::from(([0, 0, 0, 0], port))
    }

    #[test]
    fn startup_policy_allows_loopback_without_token() {
        gateway_http_startup_policy(lo(8787), None, false).unwrap();
    }

    #[test]
    fn startup_policy_rejects_non_loopback_without_token_by_default() {
        let err = gateway_http_startup_policy(any_iface(8787), None, false).unwrap_err();
        assert!(err.to_string().contains("non-loopback"));
    }

    #[test]
    fn startup_policy_allows_non_loopback_with_token() {
        gateway_http_startup_policy(any_iface(8787), Some("secret"), false).unwrap();
    }

    #[test]
    fn startup_policy_allows_non_loopback_with_insecure_override() {
        gateway_http_startup_policy(any_iface(8787), None, true).unwrap();
    }

    #[tokio::test]
    async fn gateway_mounts_artifact_routes_when_dir_is_present() {
        let dir = TempDir::new().unwrap();
        let app = build_gateway_app(None, Some(dir.path().to_path_buf()));
        let uri = "/v1/runs/run-a/artifacts?key=out.bin";

        let put = Request::builder()
            .method("PUT")
            .uri(uri)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .body(Body::from(vec![1u8, 2, 3]))
            .unwrap();
        let put_res = app.clone().oneshot(put).await.unwrap();
        assert_eq!(put_res.status(), StatusCode::NO_CONTENT);

        let get = Request::builder()
            .method("GET")
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let get_res = app.oneshot(get).await.unwrap();
        assert_eq!(get_res.status(), StatusCode::OK);
        let body = to_bytes(get_res.into_body(), usize::MAX).await.unwrap();
        assert_eq!(body.as_ref(), &[1u8, 2, 3]);
    }

    #[tokio::test]
    async fn gateway_webhook_requires_bearer_when_token_is_set() {
        let app = build_gateway_app(Some("secret".to_string()), None);
        let req = Request::builder()
            .method("POST")
            .uri("/webhook/inbound")
            .body(Body::from("{}"))
            .unwrap();
        let res = app.oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }
}
