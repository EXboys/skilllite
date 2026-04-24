//! Inbound HTTP surface for messaging webhooks (MVP), without a separate `skilllite-gateway` crate.
//!
//! Binds only when **`SKILLLITE_CHANNEL_SERVE_ALLOW=1`** (same safety pattern as `artifact-serve`).
//! Optional outbound ping: set `SKILLLITE_CHANNEL_DINGTALK_WEBHOOK` (+ optional `SKILLLITE_CHANNEL_DINGTALK_SECRET`)
//! to receive a short DingTalk text summary for each accepted POST.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header::AUTHORIZATION, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use serde_json::json;
use tower_http::trace::TraceLayer;

use crate::error::{bail, Error, Result};

/// Env var that must equal `1` before `channel serve` will bind (CLI only).
pub const CHANNEL_SERVE_ALLOW_ENV: &str = "SKILLLITE_CHANNEL_SERVE_ALLOW";

#[derive(Clone)]
struct AppState {
    bearer: Option<String>,
}

/// Run the HTTP server until process is interrupted (Ctrl+C).
pub async fn run_channel_http_server(addr: SocketAddr, bearer: Option<String>) -> Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .merge(channel_webhook_router(bearer))
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| Error::validation(format!("bind {addr}: {e}")))?;

    let local = listener
        .local_addr()
        .map_err(|e| Error::validation(e.to_string()))?;
    eprintln!("SKILLLITE_CHANNEL_HTTP_ADDR={local}");
    eprintln!(
        "channel serve: GET /health  POST /webhook/inbound  (set {}=1 to allow bind)",
        CHANNEL_SERVE_ALLOW_ENV
    );

    axum::serve(listener, app)
        .await
        .map_err(|e| Error::validation(format!("server: {e}")))
}

/// Sync entry for CLI: builds a multi-thread runtime and blocks until shutdown.
pub fn cmd_channel_serve(bind: &str, token: Option<&str>) -> Result<()> {
    let allowed = std::env::var(CHANNEL_SERVE_ALLOW_ENV)
        .map(|v| v.trim() == "1")
        .unwrap_or(false);
    if !allowed {
        bail!(
            "refusing to start channel HTTP server: set {}=1 to bind (avoids accidental network exposure)",
            CHANNEL_SERVE_ALLOW_ENV
        );
    }

    let addr: SocketAddr = bind
        .parse()
        .map_err(|e| Error::validation(format!("invalid --bind {bind:?}: {e}")))?;

    if !addr.ip().is_loopback() && token.map(str::is_empty).unwrap_or(true) {
        let insecure = std::env::var("SKILLLITE_CHANNEL_HTTP_ALLOW_INSECURE_NO_AUTH")
            .map(|v| v.trim() == "1")
            .unwrap_or(false);
        if !insecure {
            bail!(
                "non-loopback --bind requires --token or set SKILLLITE_CHANNEL_HTTP_ALLOW_INSECURE_NO_AUTH=1 (unsafe)"
            );
        }
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::validation(format!("tokio runtime: {e}")))?;
    rt.block_on(run_channel_http_server(addr, token.map(|s| s.to_string())))
}

/// Reusable inbound webhook router for embedding into a larger HTTP host.
pub fn channel_webhook_router(bearer: Option<String>) -> Router {
    let state = AppState { bearer };
    Router::new()
        .route("/webhook/inbound", post(webhook_inbound))
        .with_state(Arc::new(state))
}

async fn health() -> impl IntoResponse {
    (
        StatusCode::OK,
        axum::Json(json!({ "ok": true, "service": "skilllite-channel-serve" })),
    )
}

async fn webhook_inbound(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    if let Some(expected) = &state.bearer {
        let ok = headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.strip_prefix("Bearer ").unwrap_or(v).trim() == expected.as_str())
            .unwrap_or(false);
        if !ok {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({ "error": "unauthorized" })),
            )
                .into_response();
        }
    }

    let preview = safe_preview_body(&body);
    tracing::info!(len = body.len(), "webhook inbound");

    maybe_notify_dingtalk(&preview).await;

    (
        StatusCode::OK,
        axum::Json(json!({
            "ok": true,
            "received_bytes": body.len(),
            "preview": preview,
        })),
    )
        .into_response()
}

fn safe_preview_body(body: &Bytes) -> String {
    let s = String::from_utf8_lossy(body);
    let n = 400usize;
    if s.chars().count() <= n {
        s.into_owned()
    } else {
        s.chars().take(n).collect::<String>() + "…"
    }
}

async fn maybe_notify_dingtalk(preview: &str) {
    let url = match std::env::var("SKILLLITE_CHANNEL_DINGTALK_WEBHOOK") {
        Ok(s) if !s.trim().is_empty() => s,
        _ => return,
    };
    let secret = std::env::var("SKILLLITE_CHANNEL_DINGTALK_SECRET").ok();
    let preview = preview.to_string();
    let res = tokio::task::spawn_blocking(move || {
        let robot = skilllite_channel::DingTalkRobot::new(url, secret)?;
        let text = format!("Inbound webhook:\n```\n{preview}\n```");
        robot.send_markdown("SkillLite channel serve", &text)
    })
    .await;
    match res {
        Ok(Ok(())) => {}
        Ok(Err(e)) => tracing::warn!(error = %e, "optional dingtalk notify failed"),
        Err(e) => tracing::warn!(error = %e, "dingtalk spawn_blocking join failed"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_truncates_unicode_without_panic() {
        let s = "你好".repeat(300);
        let b = Bytes::from(s);
        let p = safe_preview_body(&b);
        assert!(p.contains('…') || p.len() <= 800);
        assert!(p.contains("你好"));
    }

    #[test]
    fn cmd_serve_refuses_without_allow_env() {
        std::env::remove_var(CHANNEL_SERVE_ALLOW_ENV);
        let r = cmd_channel_serve("127.0.0.1:0", None);
        assert!(r.is_err());
        let msg = r.expect_err("err").to_string();
        assert!(msg.contains(CHANNEL_SERVE_ALLOW_ENV), "unexpected: {msg}");
    }
}
