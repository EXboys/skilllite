//! Integration tests for evolution CLI workspace scoping.

mod common;

use common::{skilllite_bin, stdout_str};
use std::path::Path;
use std::process::{Command, Output};

fn run_with_workspace_env(args: &[&str], env_workspace: &Path) -> Output {
    Command::new(skilllite_bin())
        .args(args)
        .env("NO_COLOR", "1")
        .env("SKILLLITE_WORKSPACE", env_workspace)
        .output()
        .expect("failed to spawn skilllite")
}

fn authorize_capability(workspace: &Path, tool_name: &str) {
    let workspace_arg = workspace.to_string_lossy();
    let out = run_with_workspace_env(
        &[
            "evolution",
            "authorize-capability",
            "--json",
            "--workspace",
            workspace_arg.as_ref(),
            "--tool-name",
            tool_name,
            "--outcome",
            "failure",
            "--summary",
            "workspace scope integration seed",
        ],
        workspace,
    );
    assert!(
        out.status.success(),
        "authorize-capability failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn seed_reset_fixture(workspace: &Path, marker: &str) {
    let prompts = workspace.join("chat").join("prompts");
    std::fs::create_dir_all(prompts.join("_versions").join("txn")).expect("create prompt fixture");
    std::fs::write(prompts.join("rules.json"), marker).expect("write rules fixture");
    std::fs::write(workspace.join("chat").join("evolution.log"), marker)
        .expect("write log fixture");

    for skills_dir in ["skills", ".skills"] {
        let evolved = workspace
            .join(skills_dir)
            .join("_evolved")
            .join(format!("{marker}-{skills_dir}"));
        std::fs::create_dir_all(&evolved).expect("create evolved skill fixture");
        std::fs::write(evolved.join("SKILL.md"), marker).expect("write evolved skill fixture");
    }
}

#[test]
fn evolution_backlog_workspace_flag_overrides_env_workspace() {
    let env_workspace = tempfile::tempdir().expect("env workspace");
    let target_workspace = tempfile::tempdir().expect("target workspace");
    authorize_capability(env_workspace.path(), "env_workspace_tool");
    authorize_capability(target_workspace.path(), "target_workspace_tool");

    let target_arg = target_workspace.path().to_string_lossy();
    let out = run_with_workspace_env(
        &[
            "evolution",
            "backlog",
            "--json",
            "--hide-closed",
            "--workspace",
            target_arg.as_ref(),
            "--limit",
            "20",
        ],
        env_workspace.path(),
    );
    assert!(
        out.status.success(),
        "backlog failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let rows: Vec<serde_json::Value> =
        serde_json::from_str(stdout_str(&out).trim()).expect("valid backlog JSON");
    let notes: Vec<String> = rows
        .iter()
        .filter_map(|row| row.get("note").and_then(|note| note.as_str()))
        .map(str::to_string)
        .collect();

    assert!(
        notes
            .iter()
            .any(|note| note.contains("target_workspace_tool")),
        "target workspace backlog row should be returned: {notes:?}"
    );
    assert!(
        notes
            .iter()
            .all(|note| !note.contains("env_workspace_tool")),
        "env workspace backlog row should not leak into target query: {notes:?}"
    );
}

#[test]
fn evolution_reset_workspace_flag_isolates_all_destructive_changes() {
    let env_workspace = tempfile::tempdir().expect("env workspace");
    let target_workspace = tempfile::tempdir().expect("target workspace");
    seed_reset_fixture(env_workspace.path(), "env-marker");
    seed_reset_fixture(target_workspace.path(), "target-marker");

    let target_arg = target_workspace.path().to_string_lossy();
    let out = run_with_workspace_env(
        &[
            "evolution",
            "reset",
            "--force",
            "--workspace",
            target_arg.as_ref(),
        ],
        env_workspace.path(),
    );
    assert!(
        out.status.success(),
        "reset failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let target_rules = std::fs::read_to_string(
        target_workspace
            .path()
            .join("chat")
            .join("prompts")
            .join("rules.json"),
    )
    .expect("target rules should be reseeded");
    assert_ne!(target_rules, "target-marker");
    assert!(!target_workspace.path().join("chat/evolution.log").exists());
    assert!(!target_workspace
        .path()
        .join("chat/prompts/_versions")
        .exists());
    assert!(!target_workspace.path().join("skills/_evolved").exists());
    assert!(!target_workspace.path().join(".skills/_evolved").exists());

    let env_rules = std::fs::read_to_string(
        env_workspace
            .path()
            .join("chat")
            .join("prompts")
            .join("rules.json"),
    )
    .expect("env rules should remain");
    assert_eq!(env_rules, "env-marker");
    assert!(env_workspace.path().join("chat/evolution.log").exists());
    assert!(env_workspace.path().join("chat/prompts/_versions").exists());
    assert!(env_workspace.path().join("skills/_evolved").exists());
    assert!(env_workspace.path().join(".skills/_evolved").exists());
}
