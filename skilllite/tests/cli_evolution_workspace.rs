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
fn evolution_disable_workspace_flag_isolates_rules_mutation() {
    let env_workspace = tempfile::tempdir().expect("env workspace");
    let target_workspace = tempfile::tempdir().expect("target workspace");
    let other_workspace = tempfile::tempdir().expect("other workspace");

    let write_rules = |root: &Path, id: &str| {
        let prompts = root.join("chat").join("prompts");
        std::fs::create_dir_all(&prompts).expect("prompts dir");
        let rules =
            format!(r#"[{{"id":"{id}","mutable":true,"instruction":"{id}","disabled":false}}]"#);
        std::fs::write(prompts.join("rules.json"), rules).expect("write rules");
    };
    write_rules(env_workspace.path(), "env_rule");
    write_rules(target_workspace.path(), "target_rule");
    write_rules(other_workspace.path(), "other_rule");

    let target_arg = target_workspace.path().to_string_lossy();
    let out = run_with_workspace_env(
        &[
            "evolution",
            "disable",
            "--workspace",
            target_arg.as_ref(),
            "target_rule",
        ],
        env_workspace.path(),
    );
    assert!(
        out.status.success(),
        "disable failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let target_rules = std::fs::read_to_string(
        target_workspace
            .path()
            .join("chat")
            .join("prompts")
            .join("rules.json"),
    )
    .expect("target rules");
    assert!(
        target_rules.contains("\"disabled\": true") || target_rules.contains("\"disabled\":true"),
        "target workspace rule should be disabled: {target_rules}"
    );

    let env_rules = std::fs::read_to_string(
        env_workspace
            .path()
            .join("chat")
            .join("prompts")
            .join("rules.json"),
    )
    .expect("env rules");
    assert!(
        env_rules.contains("\"disabled\":false") || env_rules.contains("\"disabled\": false"),
        "env workspace rules must remain untouched: {env_rules}"
    );

    let other_rules = std::fs::read_to_string(
        other_workspace
            .path()
            .join("chat")
            .join("prompts")
            .join("rules.json"),
    )
    .expect("other rules");
    assert!(
        other_rules.contains("\"disabled\":false") || other_rules.contains("\"disabled\": false"),
        "unrelated workspace rules must remain untouched: {other_rules}"
    );
}
