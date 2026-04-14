//! Implementations of [`skilllite_core::artifact_store::ArtifactStore`].
//!
//! - **`local`** (default): [`LocalDirArtifactStore`] — filesystem layout `<base>/artifacts/<run_id>/<key>`.
//! - **`server`**: Axum [`artifact_router`] — see `docs/openapi/artifact-store-http-v1.yaml`.
//! - **`client`**: blocking HTTP [`HttpArtifactStore`].
//! - **Serve API** (`local` + `server`): [`run_artifact_http_server`] — `skilllite artifact-serve` (CLI requires `SKILLLITE_ARTIFACT_SERVE_ALLOW=1` to bind) or embedders calling this API directly. Startup refuses **non-loopback** binds without a bearer unless `SKILLLITE_ARTIFACT_HTTP_ALLOW_INSECURE_NO_AUTH=1`; loopback-without-token logs a **warning**. Optional `SKILLLITE_ARTIFACT_HTTP_REQUIRE_AUTH=1` requires a token even on loopback.
//!
//! The [`ArtifactStore`] trait and key validation live in `skilllite-core`. For a minimal agent build,
//! depend on `skilllite-artifact` with `default-features = false, features = ["local"]` to avoid HTTP crates.
//!
//! Fallible crate APIs (`run_artifact_http_server`, `validate_run_id`, `HttpArtifactStore::try_new`) use
//! [`Error`] / [`Result`]; [`ArtifactStore`] implementations continue to return [`skilllite_core::artifact_store::StoreError`].

#![forbid(unsafe_code)]

mod error;
mod validation;

pub use error::Error;
pub type Result<T> = error::Result<T>;

#[cfg(feature = "local")]
mod local_dir;

#[cfg(feature = "server")]
mod server;

#[cfg(feature = "client")]
mod client;

#[cfg(all(feature = "server", feature = "local"))]
mod serve;

/// Validate `run_id` for paths and HTTP (non-empty, no `..`, no `/`).
pub fn validate_run_id(run_id: &str) -> Result<()> {
    validation::validate_run_id(run_id).map_err(Error::from)
}

#[cfg(feature = "local")]
pub use local_dir::LocalDirArtifactStore;

#[cfg(feature = "server")]
pub use server::{
    artifact_router, ArtifactHttpServerConfig, ArtifactHttpState, MAX_ARTIFACT_BODY_BYTES,
};

#[cfg(feature = "client")]
pub use client::HttpArtifactStore;

#[cfg(all(feature = "server", feature = "local"))]
pub use serve::{
    run_artifact_http_server, ARTIFACT_HTTP_ALLOW_INSECURE_NO_AUTH_ENV,
    ARTIFACT_HTTP_REQUIRE_AUTH_ENV,
};
