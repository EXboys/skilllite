//! Integration tests for the `skilllite gateway` CLI surface.

mod common;

use common::{skilllite_bin, stderr_str, stdout_str};
use std::process::Command;

#[test]
fn gateway_help_succeeds() {
    let output = Command::new(skilllite_bin())
        .args(["gateway", "--help"])
        .env("NO_COLOR", "1")
        .output()
        .expect("failed to spawn skilllite gateway --help");
    assert!(output.status.success(), "gateway --help should succeed");
    let text = stdout_str(&output);
    assert!(text.contains("Serve") || text.contains("serve"));
}

#[test]
fn gateway_serve_refuses_without_allow_env() {
    let output = Command::new(skilllite_bin())
        .args(["gateway", "serve", "--bind", "127.0.0.1:0"])
        .env("NO_COLOR", "1")
        .env_remove("SKILLLITE_GATEWAY_SERVE_ALLOW")
        .output()
        .expect("failed to spawn skilllite gateway serve");
    assert!(
        !output.status.success(),
        "gateway serve should fail without allow env"
    );
    let text = stderr_str(&output);
    assert!(
        text.contains("SKILLLITE_GATEWAY_SERVE_ALLOW"),
        "unexpected stderr: {text}"
    );
}
