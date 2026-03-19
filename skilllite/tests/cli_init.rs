//! Integration tests for the `skilllite init` command.
//!
//! Tests project initialization, skill directory creation, and dependency handling.
//!
//! Note: `skilllite init` downloads skills from SKILLLITE_SKILLS_REPO when the
//! skills directory is empty. To avoid network calls in tests we pre-populate
//! the `.skills/` directory with fixtures before calling init.

mod common;

use common::{
    create_calculator_skill, create_prompt_only_skill, run_in_dir, stderr_str, stdout_str,
};

// ═══════════════════════════════════════════════════════════════════════════════
// skilllite init — with pre-existing skills (avoids network download)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn init_with_existing_skills_succeeds() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let out = run_in_dir(
        &["init", "-s", ".skills", "--skip-deps", "--skip-audit"],
        tmp.path(),
    );
    assert!(
        out.status.success(),
        "init should succeed. stderr: {}",
        stderr_str(&out)
    );
}

#[test]
fn init_skip_deps_and_audit() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let out = run_in_dir(
        &["init", "-s", ".skills", "--skip-deps", "--skip-audit"],
        tmp.path(),
    );
    assert!(out.status.success());
    let text = stderr_str(&out);
    assert!(
        text.contains("Step") || text.contains("Initializing") || text.contains("init"),
        "should show progress steps"
    );
    assert!(
        text.contains("skip") || text.contains("Skip") || text.contains("Skipping"),
        "should mention skipping deps/audit"
    );
}

#[test]
fn init_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    // Run init twice — second run should still succeed
    let out1 = run_in_dir(
        &["init", "-s", ".skills", "--skip-deps", "--skip-audit"],
        tmp.path(),
    );
    assert!(out1.status.success());

    let out2 = run_in_dir(
        &["init", "-s", ".skills", "--skip-deps", "--skip-audit"],
        tmp.path(),
    );
    assert!(
        out2.status.success(),
        "second init should also succeed. stderr: {}",
        stderr_str(&out2)
    );

    // Calculator skill should still exist
    assert!(tmp
        .path()
        .join(".skills")
        .join("calculator")
        .join("SKILL.md")
        .exists());
}

#[test]
fn init_custom_skills_dir() {
    let tmp = tempfile::tempdir().unwrap();
    // Pre-create the custom directory with a skill to avoid network download
    let custom_dir = tmp.path().join("custom-skills").join("calculator");
    std::fs::create_dir_all(custom_dir.join("scripts")).unwrap();
    std::fs::write(
        custom_dir.join("SKILL.md"),
        "---\nname: calculator\ndescription: test\n---\n# Calculator\n",
    )
    .unwrap();
    std::fs::write(
        custom_dir.join("scripts").join("main.py"),
        "#!/usr/bin/env python3\nimport json,sys\nprint(json.dumps({\"ok\":True}))\n",
    )
    .unwrap();

    let out = run_in_dir(
        &["init", "-s", "custom-skills", "--skip-deps", "--skip-audit"],
        tmp.path(),
    );
    assert!(
        out.status.success(),
        "init with custom dir should succeed. stderr: {}",
        stderr_str(&out)
    );
    assert!(
        tmp.path().join("custom-skills").exists(),
        "custom skills directory should exist"
    );
}

#[test]
fn init_reports_skill_count() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());
    create_prompt_only_skill(tmp.path());

    let out = run_in_dir(
        &["init", "-s", ".skills", "--skip-deps", "--skip-audit"],
        tmp.path(),
    );
    assert!(out.status.success());
    let text = stderr_str(&out) + &stdout_str(&out);
    assert!(
        text.contains("skill") || text.contains("Skill") || text.contains("Step"),
        "should report on skills found"
    );
}

#[test]
fn init_version_step() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let out = run_in_dir(
        &["init", "-s", ".skills", "--skip-deps", "--skip-audit"],
        tmp.path(),
    );
    assert!(out.status.success());
    let text = stderr_str(&out);
    assert!(
        text.contains("Step 1") || text.contains("binary"),
        "should show binary version step"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// skilllite clean-env
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn clean_env_dry_run() {
    let out = common::run(&["clean-env", "--dry-run"]);
    assert!(out.status.success());
}

#[test]
fn clean_env_force() {
    let out = common::run(&["clean-env", "--force"]);
    assert!(out.status.success());
}
