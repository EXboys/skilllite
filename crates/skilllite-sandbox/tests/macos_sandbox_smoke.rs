//! macOS Seatbelt (`sandbox-exec`) smoke test — exercises real Level 2 isolation on CI runners.
//!
//! Ubuntu CI uses `SKILLLITE_NO_SANDBOX=1` in `skilllite/tests/e2e_minimal.rs` because bubblewrap
//! may be absent. macOS runners always ship `sandbox-exec`, so this test validates the production
//! isolation path.

#![cfg(target_os = "macos")]

use skilllite_sandbox::runner::{
    run_in_sandbox_with_limits_and_level, ResourceLimits, RuntimePaths, SandboxConfig, SandboxLevel,
};
use std::fs;
use std::path::PathBuf;

fn resolve_python3() -> PathBuf {
    which::which("python3").expect("python3 required for macOS sandbox smoke")
}

fn write_minimal_echo_skill(dir: &std::path::Path) {
    fs::create_dir_all(dir.join("scripts")).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        r#"---
name: macos-sandbox-smoke
description: CI smoke skill for macOS Seatbelt.
license: MIT
---

# macOS sandbox smoke
"#,
    )
    .unwrap();
    fs::write(
        dir.join("scripts").join("main.py"),
        r#"#!/usr/bin/env python3
import json, sys
data = json.loads(sys.stdin.read())
print(json.dumps({"echo": data.get("message", ""), "ok": True}))
"#,
    )
    .unwrap();
}

#[test]
fn macos_seatbelt_level2_executes_minimal_python_skill() {
    std::env::remove_var("SKILLLITE_NO_SANDBOX");
    std::env::remove_var("SKILLBOX_NO_SANDBOX");
    std::env::set_var("SKILLLITE_AUTO_APPROVE", "1");

    let tmp = tempfile::tempdir().unwrap();
    let skill_dir = tmp.path().join("smoke-skill");
    write_minimal_echo_skill(&skill_dir);

    let runtime = RuntimePaths {
        python: resolve_python3(),
        node: PathBuf::from("/usr/bin/false"),
        node_modules: None,
        env_dir: PathBuf::new(),
    };
    let config = SandboxConfig {
        name: "macos-sandbox-smoke".to_string(),
        entry_point: "scripts/main.py".to_string(),
        language: "python".to_string(),
        network_enabled: false,
        network_outbound: Vec::new(),
        uses_playwright: false,
    };
    let limits = ResourceLimits {
        max_memory_mb: 256,
        timeout_secs: 30,
    };

    let output = run_in_sandbox_with_limits_and_level(
        &skill_dir,
        &runtime,
        &config,
        r#"{"message":"seatbelt-smoke"}"#,
        limits,
        SandboxLevel::Level2,
    )
    .expect("Level 2 sandbox-exec smoke should succeed");

    let parsed: serde_json::Value =
        serde_json::from_str(&output).expect("skill output must be valid JSON");
    assert_eq!(parsed["echo"], "seatbelt-smoke");
    assert_eq!(parsed["ok"], true);
}
