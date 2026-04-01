# Rust Conventions (Hard Rules)

Scope: any task that changes Rust code in the workspace.

## Must

- Use crate-level `Error` enum (via `thiserror`) + `pub type Result<T> = std::result::Result<T, Error>` in every sub-crate under `crates/`.
  Each `Error` enum must include `Other(#[from] anyhow::Error)` for gradual migration and context chaining.
- Use `use crate::Result` (not `anyhow::Result`) as the return type for all fallible functions in `crates/`.
- Use `anyhow::Context` / `.with_context()` for adding error context. The `?` operator auto-converts via `Other(#[from] anyhow::Error)`.
- Keep `cargo clippy --all-targets` at **zero warnings**. CI should enforce `-D warnings`.
- Handle all fallible operations with `?`, `match`, or `if let`. Return structured error variants for domain-specific failures.
- For user-facing string truncation/preview logic, use Unicode-safe character boundaries (`s.chars().take(n)` or equivalent).
  Never truncate with byte slicing (`&s[..n]`) unless the string is guaranteed ASCII by construction.

## Must Not

- Do not use `.unwrap()` or `.expect()` in production code paths.
  **Allowed only** inside `#[test]` functions and `#[cfg(test)]` modules.
  For cases that are logically infallible (e.g., `SystemTime::duration_since(UNIX_EPOCH)`), prefer a comment explaining why, or use a fallback value.
- Do not use `anyhow::Result`, `anyhow::bail!`, or `anyhow::anyhow!` in `crates/`.
  Use `crate::Result`, `crate::error::bail!` (crate-local macro), and `crate::Error::validation(...)` respectively.
- Do not introduce new Clippy warnings. If a Clippy lint is genuinely inapplicable, suppress it with `#[allow(...)]` and a justifying comment.
- Do not mix `thiserror` major versions within the workspace. All crates should use the same major version.
- Do not byte-slice UTF-8 text for display/error summaries (e.g., `&msg[..77]`), which can panic on CJK/emoji.

## Change Checklist

- [ ] No new `.unwrap()` / `.expect()` outside `#[test]` / `#[cfg(test)]`?
- [ ] New or changed functions return `crate::Result<T>` (not `anyhow::Result`)?
- [ ] No `anyhow::bail!` / `anyhow::anyhow!` introduced in `crates/`?
- [ ] `cargo clippy --all-targets` still zero warnings?
- [ ] If a new crate was added: does it have `error.rs` with `Error` enum + `Result<T>` + `Other(#[from] anyhow::Error)`?
- [ ] Any new truncation/preview logic is Unicode-safe (char-based), not byte-slice based?

## Quick Verify

- `cargo fmt --check`
- `cargo clippy --all-targets -D warnings`
- `cargo test`
