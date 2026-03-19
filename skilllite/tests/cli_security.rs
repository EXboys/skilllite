//! Integration tests for security scanning commands.
//!
//! Tests the `security-scan` subcommand against scripts with known patterns.
//! Note: `security-scan` validates that the script path is under the
//! allowed root (cwd), so we must run from the temp directory.

mod common;

use common::{create_safe_script, create_suspicious_script, run_in_dir, stderr_str, stdout_str};

// ═══════════════════════════════════════════════════════════════════════════════
// skilllite security-scan
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn security_scan_safe_script() {
    let tmp = tempfile::tempdir().unwrap();
    let script = create_safe_script(tmp.path());
    let script_name = script.file_name().unwrap().to_str().unwrap();

    let out = run_in_dir(&["security-scan", script_name], tmp.path());
    assert!(
        out.status.success(),
        "security-scan failed. stderr: {}",
        stderr_str(&out)
    );
    let text = stdout_str(&out);
    assert!(
        text.contains("Security Scan") || text.contains("security") || text.contains("Results"),
        "should display scan header. Got: {}",
        text
    );
}

#[test]
fn security_scan_suspicious_script_detects_issues() {
    let tmp = tempfile::tempdir().unwrap();
    let script = create_suspicious_script(tmp.path());
    let script_name = script.file_name().unwrap().to_str().unwrap();

    let out = run_in_dir(&["security-scan", script_name], tmp.path());
    assert!(
        out.status.success(),
        "security-scan should succeed. stderr: {}",
        stderr_str(&out)
    );
    let text = stdout_str(&out);
    assert!(
        text.contains("network")
            || text.contains("socket")
            || text.contains("subprocess")
            || text.contains("os.remove")
            || text.contains("risk")
            || text.contains("warning")
            || text.contains("Warning")
            || text.contains("issue")
            || text.contains("Issue")
            || text.contains("finding")
            || text.contains("Finding")
            || text.contains("HIGH")
            || text.contains("MEDIUM")
            || text.contains("LOW"),
        "scan should detect suspicious patterns. Output: {}",
        text
    );
}

#[test]
fn security_scan_json_output() {
    let tmp = tempfile::tempdir().unwrap();
    let script = create_suspicious_script(tmp.path());
    let script_name = script.file_name().unwrap().to_str().unwrap();

    let out = run_in_dir(
        &["security-scan", script_name, "--json"],
        tmp.path(),
    );
    assert!(
        out.status.success(),
        "security-scan --json failed. stderr: {}",
        stderr_str(&out)
    );
    let text = stdout_str(&out);
    let parsed: serde_json::Value =
        serde_json::from_str(text.trim()).expect("--json should produce valid JSON");
    assert!(parsed.is_object() || parsed.is_array());
}

#[test]
fn security_scan_with_allow_network() {
    let tmp = tempfile::tempdir().unwrap();
    let script = create_suspicious_script(tmp.path());
    let script_name = script.file_name().unwrap().to_str().unwrap();

    let out = run_in_dir(
        &["security-scan", script_name, "--allow-network", "--json"],
        tmp.path(),
    );
    assert!(
        out.status.success(),
        "failed. stderr: {}",
        stderr_str(&out)
    );
    let text = stdout_str(&out);
    let _parsed: serde_json::Value =
        serde_json::from_str(text.trim()).expect("should produce valid JSON");
}

#[test]
fn security_scan_nonexistent_file_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let out = run_in_dir(
        &["security-scan", "nonexistent_script_xyz.py"],
        tmp.path(),
    );
    assert!(!out.status.success());
}

#[test]
fn security_scan_with_all_allows() {
    let tmp = tempfile::tempdir().unwrap();
    let script = create_suspicious_script(tmp.path());
    let script_name = script.file_name().unwrap().to_str().unwrap();

    let out = run_in_dir(
        &[
            "security-scan",
            script_name,
            "--allow-network",
            "--allow-file-ops",
            "--allow-process-exec",
            "--json",
        ],
        tmp.path(),
    );
    assert!(
        out.status.success(),
        "failed. stderr: {}",
        stderr_str(&out)
    );
    let text = stdout_str(&out);
    let parsed: serde_json::Value = serde_json::from_str(text.trim()).unwrap();
    assert!(parsed.is_object() || parsed.is_array());
}
