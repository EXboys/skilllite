//! Integration tests for evolution CLI workspace scoping.

mod common;

use common::{create_calculator_skill, skilllite_bin, stdout_str};
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

fn write_file(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("file parent")).expect("create parent");
    std::fs::write(path, content).expect("write file");
}

fn run_reset(args: &[&str], current_dir: &Path, env_workspace: &Path) -> Output {
    Command::new(skilllite_bin())
        .args(args)
        .current_dir(current_dir)
        .env("NO_COLOR", "1")
        .env("SKILLLITE_WORKSPACE", env_workspace)
        .output()
        .expect("failed to spawn skilllite")
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
fn evolution_reset_defaults_to_current_workspace_roots() {
    let env_workspace = tempfile::tempdir().expect("env workspace");
    let target_workspace = tempfile::tempdir().expect("target workspace");

    let env_rules = env_workspace.path().join("chat/prompts/rules.json");
    let target_rules = target_workspace.path().join("chat/prompts/rules.json");
    write_file(&env_rules, "env rules must remain");
    write_file(&target_rules, "target evolved rules");
    std::fs::create_dir_all(target_workspace.path().join("skills/_evolved/generated"))
        .expect("create evolved skills");

    let out = run_reset(
        &["evolution", "reset", "--force"],
        target_workspace.path(),
        env_workspace.path(),
    );

    assert!(
        out.status.success(),
        "reset failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&env_rules).expect("env rules still exist"),
        "env rules must remain"
    );
    assert_ne!(
        std::fs::read_to_string(&target_rules).expect("target rules reseeded"),
        "target evolved rules"
    );
    assert!(
        !target_workspace.path().join("skills/_evolved").exists(),
        "reset should remove evolved skills under the current workspace skills root"
    );
}

#[test]
fn evolution_reset_workspace_flag_overrides_env_workspace_roots() {
    let env_workspace = tempfile::tempdir().expect("env workspace");
    let target_workspace = tempfile::tempdir().expect("target workspace");
    let cwd = tempfile::tempdir().expect("cwd");

    let env_rules = env_workspace.path().join("chat/prompts/rules.json");
    let target_rules = target_workspace.path().join("chat/prompts/rules.json");
    write_file(&env_rules, "env rules must remain");
    write_file(&target_rules, "target evolved rules");
    std::fs::create_dir_all(target_workspace.path().join("skills/_evolved/generated"))
        .expect("create evolved skills");

    let target_arg = target_workspace.path().to_string_lossy();
    let out = run_reset(
        &[
            "evolution",
            "reset",
            "--force",
            "--workspace",
            target_arg.as_ref(),
        ],
        cwd.path(),
        env_workspace.path(),
    );

    assert!(
        out.status.success(),
        "reset failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(&env_rules).expect("env rules still exist"),
        "env rules must remain"
    );
    assert_ne!(
        std::fs::read_to_string(&target_rules).expect("target rules reseeded"),
        "target evolved rules"
    );
    assert!(
        !target_workspace.path().join("skills/_evolved").exists(),
        "reset should remove evolved skills under the explicit workspace skills root"
    );
}

#[test]
fn evolution_repair_skills_prefers_workspace_skills_over_legacy() {
    let workspace = tempfile::tempdir().expect("workspace");
    let cwd = tempfile::tempdir().expect("cwd");
    std::fs::create_dir_all(workspace.path().join("skills")).expect("create modern skills root");
    create_calculator_skill(workspace.path());

    let workspace_arg = workspace.path().to_string_lossy();
    let out = Command::new(skilllite_bin())
        .args([
            "evolution",
            "repair-skills",
            "--workspace",
            workspace_arg.as_ref(),
            "--from-source",
        ])
        .current_dir(cwd.path())
        .env("NO_COLOR", "1")
        .env("API_KEY", "sk-test")
        .env("OPENAI_API_KEY", "sk-test")
        .output()
        .expect("failed to spawn skilllite");

    assert!(
        out.status.success(),
        "repair-skills should validate the empty modern skills root instead of legacy .skills: stdout={}, stderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout_str(&out).contains("所有技能验证通过"),
        "repair-skills stdout should report no repairs needed: {}",
        stdout_str(&out)
    );
}
