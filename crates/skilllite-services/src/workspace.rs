//! `WorkspaceService` — entry-neutral skills-directory resolution.
//!
//! Replaces three near-identical wrappers around
//! [`skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback`]:
//!
//! - `crates/skilllite-commands/src/skill/common.rs::resolve_skills_dir`
//! - `crates/skilllite-commands/src/init.rs::resolve_path_with_legacy_fallback`
//! - `crates/skilllite-commands/src/ide.rs::resolve_skills_dir_with_legacy_fallback`
//!
//! Each of those previously printed conflict warnings to stderr via
//! `eprintln!` (CLI-appropriate but not portable to other entries). The
//! Desktop bridge (`integrations/shared.rs::resolve_workspace_skills_root`)
//! used the same core call but silently dropped the warning entirely —
//! a behavioural divergence the service collapses by returning the warning
//! as structured data the entry chooses how to surface.
//!
//! # Sync interface (D3 exception, documented)
//!
//! Phase 0 D3 sets the default service interface to `async`. This service is
//! deliberately sync because every operation is purely local filesystem
//! (no network, no spawn, no long-running blocking). Forcing `async fn`
//! would only push CLI commands into `block_on` boilerplate without
//! any concurrency benefit. Future services that genuinely involve
//! async I/O (e.g. `RuntimeService` provisioning downloads) follow D3
//! and use `async fn`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use skilllite_core::skill::discovery;

use crate::error::{Error, Result};

/// Request to resolve the effective skills directory under a workspace root.
///
/// `serde`-serializable per Phase 0 D5 so future MCP / Python entries can
/// reuse the same shape across the wire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveSkillsDirRequest {
    /// Absolute path to the workspace root. Each entry is responsible for
    /// computing this (e.g. CLI uses `current_dir`, Desktop walks up from a
    /// user-selected workspace, MCP uses the server-side context). The
    /// service does **not** infer it from the process environment.
    pub workspace_root: PathBuf,

    /// Skills directory argument as provided by the caller. May be a
    /// relative name (e.g. `"skills"`) or an absolute path. The legacy
    /// fallback `skills -> .skills` only applies for the default values
    /// `"skills"` / `"./skills"`.
    pub skills_dir_arg: String,
}

/// Result of resolving the skills directory.
///
/// Exposes the same shape as
/// [`skilllite_core::skill::discovery::SkillsDirResolution`], plus the
/// formatted `conflict_warning` so non-CLI entries can render or ignore it
/// without reaching back into core.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveSkillsDirResponse {
    /// The path the caller would have referenced literally (`workspace_root
    /// + skills_dir_arg`, or the absolute path verbatim).
    pub requested_path: PathBuf,
    /// The path that should actually be used. Equals `requested_path` unless
    /// the legacy `skills -> .skills` fallback fired.
    pub effective_path: PathBuf,
    /// True when the legacy fallback was applied.
    pub used_legacy_fallback: bool,
    /// Skill names that exist in **both** `skills/` and `.skills/`. Sorted.
    pub conflicting_skill_names: Vec<String>,
    /// Pre-formatted warning string (suitable for stderr display). `None`
    /// when there are no conflicts. Non-CLI entries may inspect
    /// `conflicting_skill_names` directly instead.
    pub conflict_warning: Option<String>,
}

/// Stateless façade for workspace-related read operations.
///
/// Construct via `WorkspaceService::new()`. The struct is currently empty;
/// holding a singleton lets future TASKs add caches or injected ports
/// without changing the call shape at every site.
#[derive(Debug, Default, Clone, Copy)]
pub struct WorkspaceService;

impl WorkspaceService {
    /// Build a fresh service handle. No I/O.
    pub const fn new() -> Self {
        WorkspaceService
    }

    /// Resolve the effective skills directory for the given request.
    ///
    /// Wraps
    /// [`skilllite_core::skill::discovery::resolve_skills_dir_with_legacy_fallback`]
    /// and surfaces the duplicate-name conflict (if any) as both the raw
    /// list and a formatted message. Performs no `eprintln!` of its own —
    /// the entry adapter decides whether to print, log, or display.
    pub fn resolve_skills_dir(
        &self,
        request: ResolveSkillsDirRequest,
    ) -> Result<ResolveSkillsDirResponse> {
        let workspace_root = request.workspace_root.as_path();
        if workspace_root.as_os_str().is_empty() {
            return Err(Error::InvalidArgument(
                "workspace_root must not be empty".to_string(),
            ));
        }
        let skills_dir_arg = request.skills_dir_arg.trim();
        if skills_dir_arg.is_empty() {
            return Err(Error::InvalidArgument(
                "skills_dir_arg must not be empty".to_string(),
            ));
        }
        Ok(resolve_into_response(workspace_root, skills_dir_arg))
    }

    /// Convenience for the very common case: caller already has a
    /// `&str` workspace path (e.g. CLI `current_dir` or Desktop
    /// `find_project_root` output) and a string skills argument.
    ///
    /// Equivalent to building [`ResolveSkillsDirRequest`] manually.
    pub fn resolve_skills_dir_for_workspace(
        &self,
        workspace_root: &Path,
        skills_dir_arg: &str,
    ) -> Result<ResolveSkillsDirResponse> {
        self.resolve_skills_dir(ResolveSkillsDirRequest {
            workspace_root: workspace_root.to_path_buf(),
            skills_dir_arg: skills_dir_arg.to_string(),
        })
    }
}

fn resolve_into_response(workspace_root: &Path, skills_dir_arg: &str) -> ResolveSkillsDirResponse {
    let resolution =
        discovery::resolve_skills_dir_with_legacy_fallback(workspace_root, skills_dir_arg);
    let conflict_warning = resolution.conflict_warning();
    ResolveSkillsDirResponse {
        requested_path: resolution.requested_path,
        effective_path: resolution.effective_path,
        used_legacy_fallback: resolution.used_legacy_fallback,
        conflicting_skill_names: resolution.conflicting_skill_names,
        conflict_warning,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_empty_workspace_root() {
        let svc = WorkspaceService::new();
        let err = svc
            .resolve_skills_dir(ResolveSkillsDirRequest {
                workspace_root: PathBuf::new(),
                skills_dir_arg: "skills".into(),
            })
            .unwrap_err();
        match err {
            Error::InvalidArgument(msg) => assert!(msg.contains("workspace_root")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn rejects_blank_skills_dir_arg() {
        let svc = WorkspaceService::new();
        let tmp = tempfile::tempdir().unwrap();
        let err = svc
            .resolve_skills_dir_for_workspace(tmp.path(), "   ")
            .unwrap_err();
        match err {
            Error::InvalidArgument(msg) => assert!(msg.contains("skills_dir_arg")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn returns_legacy_fallback_when_only_dot_skills_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let legacy = tmp.path().join(".skills");
        fs::create_dir_all(&legacy).unwrap();

        let svc = WorkspaceService::new();
        let resp = svc
            .resolve_skills_dir_for_workspace(tmp.path(), "skills")
            .unwrap();
        assert!(resp.used_legacy_fallback);
        assert_eq!(resp.effective_path, legacy);
        assert!(resp.conflicting_skill_names.is_empty());
        assert!(resp.conflict_warning.is_none());
    }

    #[test]
    fn returns_requested_path_when_skills_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let primary = tmp.path().join("skills");
        fs::create_dir_all(primary.join("only-here")).unwrap();
        fs::write(
            primary.join("only-here").join("SKILL.md"),
            "name: only-here\n",
        )
        .unwrap();

        let svc = WorkspaceService::new();
        let resp = svc
            .resolve_skills_dir_for_workspace(tmp.path(), "skills")
            .unwrap();
        assert!(!resp.used_legacy_fallback);
        assert_eq!(resp.effective_path, primary);
        assert!(resp.conflict_warning.is_none());
    }

    #[test]
    fn surfaces_conflict_warning_when_duplicate_skills_in_both_roots() {
        let tmp = tempfile::tempdir().unwrap();
        let primary = tmp.path().join("skills");
        let legacy = tmp.path().join(".skills");
        fs::create_dir_all(primary.join("dup")).unwrap();
        fs::create_dir_all(legacy.join("dup")).unwrap();
        fs::write(primary.join("dup").join("SKILL.md"), "name: dup\n").unwrap();
        fs::write(legacy.join("dup").join("SKILL.md"), "name: dup\n").unwrap();

        let svc = WorkspaceService::new();
        let resp = svc
            .resolve_skills_dir_for_workspace(tmp.path(), "skills")
            .unwrap();
        assert_eq!(resp.conflicting_skill_names, vec!["dup".to_string()]);
        let warning = resp.conflict_warning.expect("warning must be set");
        assert!(warning.contains("dup"));
        assert!(warning.contains("skills/"));
        assert!(warning.contains(".skills/"));
    }

    #[test]
    fn absolute_path_skips_legacy_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let abs = tmp.path().join("custom-skills");
        fs::create_dir_all(&abs).unwrap();

        let svc = WorkspaceService::new();
        let resp = svc
            .resolve_skills_dir_for_workspace(tmp.path(), abs.to_str().unwrap())
            .unwrap();
        assert!(!resp.used_legacy_fallback);
        assert_eq!(resp.effective_path, abs);
        assert!(resp.conflicting_skill_names.is_empty());
    }
}
