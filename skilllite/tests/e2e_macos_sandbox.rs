//! macOS CLI smoke: `skilllite run --sandbox-level 2` through real Seatbelt isolation.
//!
//! Complements `e2e_minimal.rs` (Ubuntu, no sandbox) and `macos_sandbox_smoke.rs` (crate-level).

#![cfg(target_os = "macos")]

mod common;

use common::{skilllite_bin, stderr_str, stdout_str};
use std::path::Path;
use std::process::Command;

fn write_echo_skill(staging: &Path) {
    std::fs::create_dir_all(staging.join("scripts")).unwrap();
    std::fs::write(
        staging.join("SKILL.md"),
        r#"---
name: e2e-macos-seatbelt
description: Minimal echo skill for macOS sandbox CI.
license: MIT
---

# macOS Seatbelt E2E
"#,
    )
    .unwrap();
    std::fs::write(
        staging.join("scripts").join("main.py"),
        r#"#!/usr/bin/env python3
import json, sys
data = json.loads(sys.stdin.read())
print(json.dumps({"echo": data.get("message", ""), "ok": True}))
"#,
    )
    .unwrap();
}

fn run_skilllite_sandboxed(args: &[&str], dir: &Path) -> std::process::Output {
    Command::new(skilllite_bin())
        .args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .env("SKILLLITE_AUTO_APPROVE", "1")
        .env("SKILLLITE_AUDIT_DISABLED", "1")
        .output()
        .expect("failed to spawn skilllite")
}

#[test]
fn e2e_macos_run_skill_level2_seatbelt() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let staging = root.join("staging-macos-seatbelt");
    write_echo_skill(&staging);

    let out = run_skilllite_sandboxed(
        &[
            "add",
            staging.to_str().unwrap(),
            "--scan-offline",
            "-s",
            ".skills",
            "--force",
        ],
        root,
    );
    assert!(
        out.status.success(),
        "add failed: stdout={}\nstderr={}",
        stdout_str(&out),
        stderr_str(&out)
    );

    let installed = root.join(".skills").join("e2e-macos-seatbelt");
    assert!(installed.join("SKILL.md").is_file());

    let sp = installed.to_str().unwrap();
    let out = run_skilllite_sandboxed(
        &[
            "run",
            sp,
            r#"{"message":"cli-seatbelt-smoke"}"#,
            "--sandbox-level",
            "2",
        ],
        root,
    );
    assert!(
        out.status.success(),
        "run failed: stdout={}\nstderr={}",
        stdout_str(&out),
        stderr_str(&out)
    );

    let combined = stdout_str(&out) + &stderr_str(&out);
    assert!(
        combined.contains("cli-seatbelt-smoke"),
        "expected echoed payload in output: {}",
        combined
    );
}
