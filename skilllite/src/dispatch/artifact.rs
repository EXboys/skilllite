//! `skilllite artifact-serve` — HTTP API for run-scoped artifacts.
//!
//! Refuses to bind unless **`SKILLLITE_ARTIFACT_SERVE_ALLOW=1`** is set, so the subcommand can ship
//! in the default binary without accidentally opening a listener. Embedders calling
//! `skilllite_artifact::run_artifact_http_server` directly are unaffected.

use std::path::PathBuf;

use crate::cli::Commands;
use crate::command_registry::CommandRegistry;
use crate::Error;

/// Env var that must equal `1` before `artifact-serve` will bind (CLI only).
pub const ARTIFACT_SERVE_ALLOW_ENV: &str = "SKILLLITE_ARTIFACT_SERVE_ALLOW";

pub fn register(reg: &mut CommandRegistry) {
    reg.register(|cmd| {
        if let Commands::ArtifactServe { dir, bind, token } = cmd {
            Some(run_artifact_serve(dir.clone(), bind.clone(), token.clone()))
        } else {
            None
        }
    });
}

fn run_artifact_serve(dir: PathBuf, bind: String, token: Option<String>) -> Result<(), Error> {
    let allowed = std::env::var(ARTIFACT_SERVE_ALLOW_ENV)
        .map(|v| v.trim() == "1")
        .unwrap_or(false);
    if !allowed {
        return Err(Error::msg(format!(
            "refusing to start artifact HTTP server: set {ARTIFACT_SERVE_ALLOW_ENV}=1 to bind (avoids accidental network exposure)"
        )));
    }
    let addr: std::net::SocketAddr = bind
        .parse()
        .map_err(|e| Error::msg(format!("invalid --bind {bind:?}: {e}")))?;
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| Error::msg(format!("tokio runtime: {e}")))?;
    rt.block_on(skilllite_artifact::run_artifact_http_server(
        dir, addr, token,
    ))
    .map_err(|e| Error::msg(e.to_string()))
}
