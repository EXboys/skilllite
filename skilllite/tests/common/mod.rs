//! Shared test utilities for integration tests.
//!
//! Provides helpers to locate the compiled binary, run CLI commands,
//! and create temporary skill fixtures for isolated testing.

#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// Locate the compiled `skilllite` binary adjacent to the test executable.
pub fn skilllite_bin() -> PathBuf {
    let mut path = std::env::current_exe()
        .expect("cannot determine test binary path")
        .parent()
        .expect("no parent dir for test binary")
        .parent()
        .expect("no deps parent dir")
        .to_path_buf();
    path.push("skilllite");
    if cfg!(windows) {
        path.set_extension("exe");
    }
    assert!(
        path.exists(),
        "skilllite binary not found at {}. Run `cargo build` first.",
        path.display()
    );
    path
}

/// Run `skilllite <args>` and return the full output.
pub fn run(args: &[&str]) -> Output {
    Command::new(skilllite_bin())
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to spawn skilllite")
}

/// Run `skilllite <args>` inside a specific working directory.
pub fn run_in_dir(args: &[&str], dir: &Path) -> Output {
    Command::new(skilllite_bin())
        .args(args)
        .current_dir(dir)
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to spawn skilllite")
}

/// Run `skilllite <args>`, feed `stdin_data` via stdin, and return output.
pub fn run_with_stdin(args: &[&str], stdin_data: &str) -> Output {
    let mut child = Command::new(skilllite_bin())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1")
        .spawn()
        .expect("failed to spawn skilllite");

    if let Some(ref mut stdin) = child.stdin {
        stdin
            .write_all(stdin_data.as_bytes())
            .expect("failed to write stdin");
    }
    drop(child.stdin.take());
    child.wait_with_output().expect("failed to wait on child")
}

/// Run `skilllite <args>` inside `dir`, feed `stdin_data`, and return output.
pub fn run_in_dir_with_stdin(args: &[&str], dir: &Path, stdin_data: &str) -> Output {
    let mut child = Command::new(skilllite_bin())
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1")
        .spawn()
        .expect("failed to spawn skilllite");

    if let Some(ref mut stdin) = child.stdin {
        stdin
            .write_all(stdin_data.as_bytes())
            .expect("failed to write stdin");
    }
    drop(child.stdin.take());
    child.wait_with_output().expect("failed to wait on child")
}

/// Collect stdout as a UTF-8 string.
pub fn stdout_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Collect stderr as a UTF-8 string.
pub fn stderr_str(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

// ─── Fixture builders ────────────────────────────────────────────────────────

/// Create a calculator skill fixture under `<dir>/.skills/calculator/`.
pub fn create_calculator_skill(dir: &Path) {
    let skill_dir = dir.join(".skills").join("calculator");
    std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();

    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: calculator
description: A simple calculator that performs arithmetic operations.
license: MIT
capabilities: ["calc", "math"]
metadata:
  author: test
  version: "1.0"
---

# Calculator Skill

A simple calculator.
"#,
    )
    .unwrap();

    std::fs::write(
        skill_dir.join("scripts").join("main.py"),
        r#"#!/usr/bin/env python3
import json, sys

def main():
    data = json.loads(sys.stdin.read())
    op = data.get("operation", "add")
    a, b = float(data.get("a", 0)), float(data.get("b", 0))
    if op == "add":
        result = a + b
    elif op == "subtract":
        result = a - b
    elif op == "multiply":
        result = a * b
    elif op == "divide":
        if b == 0:
            print(json.dumps({"error": "Division by zero"}))
            return
        result = a / b
    else:
        print(json.dumps({"error": f"Unknown operation: {op}"}))
        return
    print(json.dumps({"operation": op, "a": a, "b": b, "result": result}))

if __name__ == "__main__":
    main()
"#,
    )
    .unwrap();
}

/// Create a prompt-only skill (no entry_point) under `<dir>/.skills/prompt-helper/`.
pub fn create_prompt_only_skill(dir: &Path) {
    let skill_dir = dir.join(".skills").join("prompt-helper");
    std::fs::create_dir_all(&skill_dir).unwrap();

    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: prompt-helper
description: A prompt-only skill for testing.
license: MIT
---

# Prompt Helper

This skill provides instructions only, with no executable entry point.
"#,
    )
    .unwrap();
}

/// Create a script with suspicious patterns for security-scan testing.
pub fn create_suspicious_script(dir: &Path) -> PathBuf {
    let script = dir.join("suspicious.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import os
import subprocess
import socket

# Network call
sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.connect(("evil.example.com", 80))

# File system access
os.remove("/etc/important")

# Process execution
subprocess.run(["rm", "-rf", "/"])
"#,
    )
    .unwrap();
    script
}

/// Create a safe, minimal script for security-scan testing.
pub fn create_safe_script(dir: &Path) -> PathBuf {
    let script = dir.join("safe.py");
    std::fs::write(
        &script,
        r#"#!/usr/bin/env python3
import json, sys

data = json.loads(sys.stdin.read())
print(json.dumps({"result": data.get("x", 0) + 1}))
"#,
    )
    .unwrap();
    script
}

/// Create a skill with an entry_point that has a requirements.txt.
pub fn create_skill_with_deps(dir: &Path) {
    let skill_dir = dir.join(".skills").join("with-deps");
    std::fs::create_dir_all(skill_dir.join("scripts")).unwrap();

    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: with-deps
description: Skill with dependencies for testing.
license: MIT
compatibility: Requires Python 3.x
metadata:
  author: test
  version: "1.0"
---

# Skill With Dependencies
"#,
    )
    .unwrap();

    std::fs::write(
        skill_dir.join("scripts").join("main.py"),
        r#"#!/usr/bin/env python3
import json, sys
print(json.dumps({"ok": True}))
"#,
    )
    .unwrap();

    std::fs::write(skill_dir.join("requirements.txt"), "requests==2.31.0\n").unwrap();
}
