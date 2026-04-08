//! Run the artifact HTTP listener (shared by CLI and tests).

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use skilllite_core::artifact_store::ArtifactStore;

use crate::Result;
use crate::{artifact_router, ArtifactHttpServerConfig, ArtifactHttpState, LocalDirArtifactStore};

/// Bind, print `SKILLLITE_ARTIFACT_HTTP_ADDR=...` to stdout, then serve until the process exits.
pub async fn run_artifact_http_server(
    data_dir: PathBuf,
    bind: std::net::SocketAddr,
    bearer_token: Option<String>,
) -> Result<()> {
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
