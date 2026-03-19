//! Integration tests for basic CLI behavior: help, version, subcommand routing.

mod common;

use common::{run, stderr_str, stdout_str};

// ─── --help ──────────────────────────────────────────────────────────────────

#[test]
fn help_flag_succeeds() {
    let out = run(&["--help"]);
    assert!(out.status.success(), "exit code should be 0");
    let text = stdout_str(&out);
    assert!(
        text.contains("skilllite"),
        "help should mention the program name"
    );
}

#[test]
fn help_flag_shows_subcommands() {
    let out = run(&["--help"]);
    let text = stdout_str(&out);
    for sub in &["run", "scan", "validate", "list", "mcp", "init"] {
        assert!(
            text.contains(sub),
            "help output should list the `{}` subcommand",
            sub
        );
    }
}

// ─── --version ───────────────────────────────────────────────────────────────

#[test]
fn version_flag_prints_version() {
    let out = run(&["--version"]);
    assert!(out.status.success());
    let text = stdout_str(&out);
    assert!(
        text.contains("skilllite"),
        "version output should contain binary name"
    );
    // Version string should match semver pattern (e.g. "0.1.15")
    assert!(
        text.chars().any(|c| c.is_ascii_digit()),
        "version output should contain digits"
    );
}

// ─── Subcommand --help ───────────────────────────────────────────────────────

#[test]
fn run_help_succeeds() {
    let out = run(&["run", "--help"]);
    assert!(out.status.success());
    let text = stdout_str(&out);
    assert!(text.contains("SKILL_DIR") || text.contains("skill"));
}

#[test]
fn scan_help_succeeds() {
    let out = run(&["scan", "--help"]);
    assert!(out.status.success());
    let text = stdout_str(&out);
    assert!(text.contains("SKILL_DIR") || text.contains("skill"));
}

#[test]
fn list_help_succeeds() {
    let out = run(&["list", "--help"]);
    assert!(out.status.success());
    let text = stdout_str(&out);
    assert!(text.contains("skills") || text.contains("json"));
}

#[test]
fn init_help_succeeds() {
    let out = run(&["init", "--help"]);
    assert!(out.status.success());
    let text = stdout_str(&out);
    assert!(text.contains("init") || text.contains("skills"));
}

#[test]
fn validate_help_succeeds() {
    let out = run(&["validate", "--help"]);
    assert!(out.status.success());
}

#[test]
fn show_help_succeeds() {
    let out = run(&["show", "--help"]);
    assert!(out.status.success());
}

#[test]
fn verify_help_succeeds() {
    let out = run(&["verify", "--help"]);
    assert!(out.status.success());
}

#[test]
fn security_scan_help_succeeds() {
    let out = run(&["security-scan", "--help"]);
    assert!(out.status.success());
}

#[test]
fn mcp_help_succeeds() {
    let out = run(&["mcp", "--help"]);
    assert!(out.status.success());
}

#[test]
fn clean_env_help_succeeds() {
    let out = run(&["clean-env", "--help"]);
    assert!(out.status.success());
}

#[test]
fn reindex_help_succeeds() {
    let out = run(&["reindex", "--help"]);
    assert!(out.status.success());
}

// ─── Error cases ─────────────────────────────────────────────────────────────

#[test]
fn missing_subcommand_shows_help() {
    let out = run(&[]);
    assert!(!out.status.success(), "no subcommand should fail");
    let text = stderr_str(&out);
    assert!(
        text.contains("Usage") || text.contains("usage") || text.contains("skilllite"),
        "should show usage hint"
    );
}

#[test]
fn unknown_subcommand_fails() {
    let out = run(&["nonexistent-command-xyz"]);
    assert!(!out.status.success());
}

// ─── Subcommand aliases ──────────────────────────────────────────────────────

#[test]
fn ls_is_alias_for_list() {
    let out = run(&["ls", "--help"]);
    assert!(out.status.success(), "`ls` should be accepted as alias for `list`");
}
