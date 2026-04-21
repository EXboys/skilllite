//! `skilllite-services` — shared application service layer.
//!
//! This crate hosts entry-neutral use case orchestration consumed by both the
//! CLI (`skilllite`) and the Desktop (`skilllite-assistant`) entries, with
//! the future MCP entry as a planned consumer. It exists to eliminate
//! duplication between `crates/skilllite-commands/src/**` and
//! `crates/skilllite-assistant/src-tauri/src/skilllite_bridge/**`.
//!
//! # Status
//!
//! Phase 1A (TASK-2026-044, 2026-04-20). First real service:
//! [`workspace::WorkspaceService`]. Future Phase 1B introduces the async
//! [`workspace`] is sync because all its operations are local filesystem
//! reads with no network or long-running I/O — see the module docs for a
//! detailed rationale of this Phase 0 D3 exception. Future services that
//! genuinely involve async I/O (e.g. runtime provisioning) will use
//! `async fn` per D3.
//!
//! # Boundaries (per `spec/architecture-boundaries.md` and Phase 0 D1..D5)
//!
//! - Inputs and outputs are `serde`-serializable plain data types; do not
//!   leak Tauri, `tokio::sync`, or platform-specific types across the
//!   interface.
//! - Errors use per-crate `thiserror` ([`error::Error`]); entry adapters
//!   convert to `anyhow::Result` / structured Tauri errors at the boundary.
//! - Allowed direct dependents are entry-layer crates only:
//!   `skilllite`, `skilllite-commands`, `skilllite-assistant`. Domain crates
//!   (`skilllite-{core,fs,sandbox,executor,agent,evolution,artifact,swarm}`)
//!   must not depend on this crate.
//!
//! See `todo/multi-entry-service-layer-refactor-plan.md` for the full plan.

#![forbid(unsafe_code)]
#![deny(rust_2018_idioms)]
#![warn(missing_docs)]

pub mod error;
pub mod workspace;

pub use error::{Error, Result};
pub use workspace::{ResolveSkillsDirRequest, ResolveSkillsDirResponse, WorkspaceService};
