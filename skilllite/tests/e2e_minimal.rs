//! Minimal end-to-end: `skilllite add` (local path) → `scan` → `run`.
//!
//! Uses an isolated temp workspace so it never touches the real `~/.skills`.

mod common;

use common::{skilllite_bin, stderr_str, stdout_str};
use std::path::Path;
use std::process::Command;

fn write_echo_skill(staging: &Path) {
    std::fs::create_dir_all(staging.join("scripts")).unwrap();
    std::fs::write(
        staging.join("SKILL.md"),
        r#"---
name: e2e-echo-skill
description: Minimal echo skill for CI E2E.
license: MIT
---

# E2E Echo

Reads JSON from stdin and echoes `message` back with `ok: true`.
"#,
    )
    .unwrap();
    std::fs::write(
        staging.join("scripts").join("main.py"),
        r#"#!/usr/bin/env python3
import json, sys
data = json.loads(sys.stdin.read())
msg = data.get("message", "")
print(json.dumps({"echo": msg, "ok": True}))
"#,
    )
    .unwrap();
}

fn run_skilllite_in_dir(args: &[&str], dir: &Path) -> std::process::Output {
    let mut cmd = Command::new(skilllite_bin());
    cmd.args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        // CI may lack bubblewrap; default Linux policy refuses without bwrap unless
        // SKILLLITE_ALLOW_LINUX_NAMESPACE_FALLBACK=1 — use full opt-out for stable E2E.
        .env("SKILLLITE_NO_SANDBOX", "1")
        .env("SKILLLITE_AUTO_APPROVE", "1")
        .env("SKILLLITE_AUDIT_DISABLED", "1");
    cmd.output().expect("failed to spawn skilllite")
}

#[test]
fn e2e_add_scan_run_minimal_skill() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let staging = root.join("staging-e2e-echo");
    write_echo_skill(&staging);
    let src = staging.to_str().unwrap();

    // 1) Install into .skills (local path)
    let out = run_skilllite_in_dir(
        &["add", src, "--scan-offline", "-s", ".skills", "--force"],
        root,
    );
    assert!(
        out.status.success(),
        "add failed: stdout={}\nstderr={}",
        stdout_str(&out),
        stderr_str(&out)
    );

    let installed = root.join(".skills").join("e2e-echo-skill");
    assert!(
        installed.join("SKILL.md").is_file(),
        "expected skill installed at {}",
        installed.display()
    );

    // 2) Scan
    let sp = installed.to_str().unwrap();
    let out = run_skilllite_in_dir(&["scan", sp], root);
    assert!(out.status.success(), "scan failed: {}", stderr_str(&out));
    let scan: serde_json::Value =
        serde_json::from_str(stdout_str(&out).trim()).expect("scan should return JSON");
    assert_eq!(scan["has_skill_md"], true);

    // 3) Run
    let out = run_skilllite_in_dir(&["run", sp, r#"{"message":"hello-e2e"}"#], root);
    assert!(
        out.status.success(),
        "run failed: stdout={}\nstderr={}",
        stdout_str(&out),
        stderr_str(&out)
    );
    let combined = stdout_str(&out) + &stderr_str(&out);
    assert!(
        combined.contains("hello-e2e"),
        "expected echoed payload in output: {}",
        combined
    );
}
