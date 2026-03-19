//! Integration tests for skill management commands: list, show, scan, validate, verify.
//!
//! Each test creates an isolated temp directory with fixture skills so tests
//! never interfere with the real `.skills/` directory or with each other.

mod common;

use common::{
    create_calculator_skill, create_prompt_only_skill, run_in_dir, stderr_str, stdout_str,
};

// ═══════════════════════════════════════════════════════════════════════════════
// skilllite list
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn list_empty_dir_shows_no_skills() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run_in_dir(&["list", "-s", ".skills"], tmp.path());
    assert!(out.status.success());
}

#[test]
fn list_with_skills_shows_names() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let out = run_in_dir(&["list", "-s", ".skills"], tmp.path());
    assert!(out.status.success());
    let text = stderr_str(&out) + &stdout_str(&out);
    assert!(
        text.contains("calculator"),
        "list output should contain 'calculator'"
    );
}

#[test]
fn list_json_returns_valid_json() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let out = run_in_dir(&["list", "-s", ".skills", "--json"], tmp.path());
    assert!(out.status.success());
    let text = stdout_str(&out);
    let parsed: serde_json::Value =
        serde_json::from_str(text.trim()).expect("list --json should return valid JSON");
    assert!(parsed.is_array(), "JSON output should be an array");
}

#[test]
fn list_multiple_skills() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());
    create_prompt_only_skill(tmp.path());

    let out = run_in_dir(&["list", "-s", ".skills", "--json"], tmp.path());
    assert!(out.status.success());
    let text = stdout_str(&out);
    let arr: Vec<serde_json::Value> = serde_json::from_str(text.trim()).unwrap();
    assert!(
        arr.len() >= 2,
        "should list at least 2 skills, got {}",
        arr.len()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// skilllite show
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn show_existing_skill() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let out = run_in_dir(&["show", "calculator", "-s", ".skills"], tmp.path());
    assert!(out.status.success());
    let text = stderr_str(&out);
    assert!(
        text.contains("calculator") || text.contains("Calculator"),
        "show should display skill name"
    );
}

#[test]
fn show_json_returns_valid_json() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let out = run_in_dir(
        &["show", "calculator", "-s", ".skills", "--json"],
        tmp.path(),
    );
    assert!(out.status.success());
    let text = stdout_str(&out);
    let parsed: serde_json::Value =
        serde_json::from_str(text.trim()).expect("show --json should return valid JSON");
    assert!(parsed.is_object());
}

#[test]
fn show_nonexistent_skill_fails() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let out = run_in_dir(&["show", "nonexistent-skill", "-s", ".skills"], tmp.path());
    assert!(!out.status.success());
}

// ═══════════════════════════════════════════════════════════════════════════════
// skilllite scan
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn scan_skill_returns_json() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let skill_path = tmp.path().join(".skills").join("calculator");
    let out = run_in_dir(&["scan", skill_path.to_str().unwrap()], tmp.path());
    assert!(out.status.success());
    let text = stdout_str(&out);
    let parsed: serde_json::Value =
        serde_json::from_str(text.trim()).expect("scan should return valid JSON");
    assert_eq!(parsed["has_skill_md"], true);
    assert!(parsed["scripts"].is_array());
}

#[test]
fn scan_detects_entry_point() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let skill_path = tmp.path().join(".skills").join("calculator");
    let out = run_in_dir(&["scan", skill_path.to_str().unwrap()], tmp.path());
    assert!(out.status.success());
    let text = stdout_str(&out);
    let parsed: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
    assert!(parsed["skill_metadata"]["name"]
        .as_str()
        .unwrap()
        .contains("calculator"));
}

#[test]
fn scan_prompt_only_skill() {
    let tmp = tempfile::tempdir().unwrap();
    create_prompt_only_skill(tmp.path());

    let skill_path = tmp.path().join(".skills").join("prompt-helper");
    let out = run_in_dir(&["scan", skill_path.to_str().unwrap()], tmp.path());
    assert!(out.status.success());
    let text = stdout_str(&out);
    let parsed: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(parsed["has_skill_md"], true);
    assert!(
        parsed["skill_metadata"]["entry_point"].is_null(),
        "prompt-only skill should have null entry_point"
    );
}

#[test]
fn scan_nonexistent_dir_fails() {
    let out = run_in_dir(
        &["scan", "/nonexistent/path/xyz"],
        std::path::Path::new("/tmp"),
    );
    assert!(!out.status.success());
}

// ═══════════════════════════════════════════════════════════════════════════════
// skilllite validate
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn validate_valid_skill_passes() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let skill_path = tmp.path().join(".skills").join("calculator");
    let out = run_in_dir(&["validate", skill_path.to_str().unwrap()], tmp.path());
    assert!(out.status.success());
    let text = stdout_str(&out);
    assert!(
        text.contains("passed") || text.contains("Passed") || text.contains("valid"),
        "validate should report success"
    );
}

#[test]
fn validate_nonexistent_dir_fails() {
    let out = run_in_dir(
        &["validate", "/tmp/nonexistent-skill-dir-xyz"],
        std::path::Path::new("/tmp"),
    );
    assert!(!out.status.success());
}

// ═══════════════════════════════════════════════════════════════════════════════
// skilllite verify
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn verify_skill_shows_status() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let out = run_in_dir(&["verify", "calculator", "-s", ".skills"], tmp.path());
    // verify may succeed or fail depending on manifest state, but should not crash
    let text = stderr_str(&out) + &stdout_str(&out);
    assert!(
        text.contains("OK")
            || text.contains("UNSIGNED")
            || text.contains("HASH_CHANGED")
            || text.contains("Status")
            || text.contains("status")
            || text.contains("Integrity")
            || text.contains("verify"),
        "verify should report some status"
    );
}

#[test]
fn verify_json_returns_valid_json() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let out = run_in_dir(
        &["verify", "calculator", "-s", ".skills", "--json"],
        tmp.path(),
    );
    let text = stdout_str(&out);
    if !text.trim().is_empty() {
        let parsed: serde_json::Value =
            serde_json::from_str(text.trim()).expect("verify --json should return valid JSON");
        assert!(parsed.is_object());
        assert!(
            parsed.get("status").is_some(),
            "JSON should have 'status' field"
        );
    }
}

#[test]
fn verify_nonexistent_skill_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run_in_dir(
        &["verify", "nonexistent-skill", "-s", ".skills"],
        tmp.path(),
    );
    assert!(!out.status.success());
}

// ═══════════════════════════════════════════════════════════════════════════════
// skilllite info
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn info_shows_skill_details() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());

    let skill_path = tmp.path().join(".skills").join("calculator");
    let out = run_in_dir(&["info", skill_path.to_str().unwrap()], tmp.path());
    assert!(out.status.success());
    let text = stderr_str(&out) + &stdout_str(&out);
    assert!(
        text.contains("calculator") || text.contains("Calculator"),
        "info should display skill name"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// skilllite reindex
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn reindex_with_skills() {
    let tmp = tempfile::tempdir().unwrap();
    create_calculator_skill(tmp.path());
    create_prompt_only_skill(tmp.path());

    let out = run_in_dir(&["reindex", "-s", ".skills"], tmp.path());
    assert!(out.status.success());
}

#[test]
fn reindex_empty_dir() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".skills")).unwrap();

    let out = run_in_dir(&["reindex", "-s", ".skills"], tmp.path());
    assert!(out.status.success());
}
