# Changelog

All notable changes to SkillLite are documented in this file. This log emphasizes **technical impact** (what problems we solve), **security** (defense-in-depth and safe evolution), and **architecture** (layering, boundaries, and evolvability). For full design rationale see [Architecture](docs/en/ARCHITECTURE.md) .

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Changed

- **Evolution defaults**: Coordinator **shadow mode** is now **off** by default (`SKILLLITE_EVO_SHADOW_MODE` unset ⇒ execute when policy allows), and **low-risk auto-execute** is **on** by default (`SKILLLITE_EVO_AUTO_EXECUTE_LOW_RISK`). Restore the previous conservative behavior with `SKILLLITE_EVO_SHADOW_MODE=1` and `SKILLLITE_EVO_AUTO_EXECUTE_LOW_RISK=0`.
- **A9 / dual pipeline**: `ChatSession` **does not** run evolution in-process (reverts 2026-03-01 `36cbf93` in-agent timers). **Life Pulse** spawns `skilllite evolution run` on interval + unprocessed threshold (workspace `.env`). **Desktop chat** does not inject P7 **evolution_options** bubbles (`useChatEvents.ts`). CLI-only: `skilllite evolution run` or cron.

---

## [0.1.19] - 2026-04-02

### Changed

- **Linux sandbox (fail-closed)**: If bubblewrap/firejail are missing or the strong sandbox path fails, execution is **refused by default** (aligned with Windows). Opt-in weak fallback (PID/UTS/net namespaces only) requires **`SKILLLITE_ALLOW_LINUX_NAMESPACE_FALLBACK=1`** (legacy `SKILLBOX_ALLOW_LINUX_NAMESPACE_FALLBACK`); the event is logged via `security_sandbox_fallback` with reason `linux_namespace_fallback`.

### Added

- **Evolution — prompt snapshot retention**: `SKILLLITE_EVOLUTION_SNAPSHOT_KEEP` (default `10`) controls how many `chat/prompts/_versions/<txn>/` dirs are kept; set to **`0` to disable pruning** for local, Git-free traceability (disk grows with runs).
- **Scheduled agent runs (MVP)**: `skilllite schedule tick` reads `.skilllite/schedule.json`, runs **due** jobs (`interval_seconds` per job, optional global `min_interval_seconds_between_runs` and `max_runs_per_day`), and injects each job’s `message` as **one full `chat` turn** (same agent loop as interactive chat). State is stored in `.skilllite/schedule-state.json`. Example config: `.skilllite/schedule.example.json`. Parsing and due logic live in `skilllite-core::schedule`.
- **Schedule — wall clock & payload**: Jobs may use **system-local** `daily_at` and/or **`daily_times`** (multiple `HH:MM` per day), or **`once_at`** (`YYYY-MM-DDTHH:MM`). Optional `goal` / `steps_prompt` merge with `message` for the injected user turn. **Wall-clock jobs bypass** global `min_interval_seconds_between_runs` so a failed once/daily run can retry on the next tick; **interval-only** jobs still respect it. **`schedule tick` continues** after a single job’s `run_chat` error (logs, does not update state for that job).
- **Schedule opt-in**: Non–dry-run `schedule tick` requires **`SKILLLITE_SCHEDULE_ENABLED=1`** (or `true`); if unset, due jobs are skipped with a stderr hint so cron cannot accidentally call the API. `--dry-run` is unaffected.
- **SkillLite Assistant (desktop)**: Split `skilllite_bridge` into focused modules (`protocol`, `paths`, `chat`, `transcript`, `sessions`, `workspace`, `integrations`) and added unit tests for protocol parsing, path validation, transcript path listing, and idle `stop_chat`.
- **SkillLite Assistant (desktop)**: Stricter default **CSP** for bundled webviews (with `localhost` / `ipc` / `tauri` allowances for dev and in-app IPC); **Windows-oriented** relative-path checks for memory/output reads and transcript filenames; **Chinese** tray menu and tooltip; **tray build failures** and **global shortcut registration failures** surface as in-app toasts (tray also emits `skilllite-chrome-bootstrap`).
- **SkillLite Assistant (desktop)**: Message list uses **virtual scrolling** (`@tanstack/react-virtual`) when there are many segments (threshold 48); **ErrorBoundary** adds “copy error details” (message, stack, component stack).
- **SkillLite Assistant (desktop)**: `createUpdaterArtifacts` enabled in `tauri.conf.json` to prepare for Tauri updater wiring (endpoints and signing still required in release automation).

### Release

- Workspace, Python SDK, and SkillLite Assistant (Tauri) bumped to **0.1.19**; internal crate path dependency pins aligned to **0.1.19**.

---

## [0.1.16] - 2026-03-25

### Added

- **Observability — edit audit**: Agent file-edit tools (`search_replace`, `preview_edit`, `insert_lines`) emit structured JSONL events (`edit_applied`, `edit_previewed`, `edit_failed`, `edit_inserted`) with `edit_id`, top-level `path`, `workspace`, and `context` (from `SKILLLITE_AUDIT_CONTEXT`); audit append flushes after each line.

### Changed

- **Tests**: `skilllite-agent` builtin tests disable audit at process start (`ctor` + `SKILLLITE_AUDIT_DISABLED=1`) so `cargo test` does not pollute the default audit directory.

### Desktop (SkillLite Assistant)

- Desktop 与 Rust workspace 统一为 **0.1.16**（`package.json`、`tauri.conf.json`、`src-tauri/Cargo.toml`）。

---

## [0.1.15] - 2026-03-19

### Fixed

- **PyPI package now bundles the native binary**: After `pip install skilllite`, `skilllite chat` and other CLI commands work out of the box. Previously the PyPI package did not include the skilllite binary, so `skilllite chat` had no effect. CI now builds platform-specific wheels (Linux x64, macOS Intel, macOS ARM, Windows x64), each with the matching binary.
- Improve artifact extraction logic in GitHub Actions so release assets are correctly collected.
- macOS build: conditionally skip signing and notarization when credentials are unavailable; suppress noisy certificate import output.

### Changed

- **Workspace version management**: All crate versions now inherit from `[workspace.package]` in root `Cargo.toml`. Bumping a release only requires changing one line.
- **CI: fix Windows PyPI wheel build**: Replaced inline `python -c` version injection with a standalone script (`scripts/set_pyproject_version.py`) to avoid PowerShell parsing errors on Windows runners.
- Release pipeline injects the Git tag (e.g. `v0.1.15`) into the package version, so PyPI version matches the GitHub release.
- Pre-upload check: CI verifies exactly 4 wheels + 1 sdist before publishing; job fails if any artifact is missing.
- GitHub Release is created only when the build job succeeds.
- Desktop assistant: new skill management features; macOS workflow no longer leaves sensitive API key files on disk.

### Notes

- **Upgrade**: Versions before 0.1.15 on PyPI do not include the binary. If you use `pip install skilllite`, upgrade to 0.1.15 or later.
- Linux ARM64 wheels are not yet published; on that platform you can build from source (e.g. `./scripts/build_wheels.sh`).

---

## [0.1.14] - 2026-03-18

### Added

- macOS build: signing and notarization steps for distribution.

### Changed

- Bump package versions to 0.1.14 across all crates and Python SDK.
- Refactor environment setup for better readability and structure.
- Release workflow and versioning logic updated.

---

## [0.1.13] - 2026-03-18

### Added

- Ollama model selection and improved settings management in desktop/CLI.
- Optional SOUL template creation on first run.
- Skill deduplication: same-round pending skills are deduplicated to avoid redundant execution.
- Evolution engine: configurable thresholds and profiles for evolution behavior.
- Agent identity: Law and Beliefs integration for consistent behavior.

### Changed

- Release workflow triggers only on `main` branch and when relevant paths change.
- Chat session initialization and onboarding experience improved; environment variable handling and documentation updated (including same-round deduplication in ENV_REFERENCE).
- Architecture docs: entry points and capability domains documented; security benchmark references renamed from SkillBox to SkillLite.
- Python version requirements documented (3.10+); network proxy and dependency-audit moved to optional features (smaller default binary; enable via features when needed).

### Architecture

- **Evolution vs. security**: Configurable evolution (thresholds, profiles, deduplication) still produces artifacts that must pass the same install-time scan and runtime sandbox as manually added skills — no separate code path for evolved content.

- Regex handling and error reporting in several modules; LlmClient initialization error handling; skill execution and refinement error handling; metadata and external learner tests.

---

## [0.1.12] - 2026-03-15

### Added

- Task planning: planning control tools, tool hint resolver with availability checks, and improved task execution flow.
- Agent: capability-based tool registration, read-only mode for tool execution, rule history tracking and query.
- EventSink: preview and swarm event handling, command lifecycle events; `run_command` output streams to execution logs.
- Rule retirement: evolution rules with low effectiveness or trigger count can be retired from the evolution pool (sandbox and install-time security unchanged).
- ToolResult: `counts_as_failure` flag for clearer error handling in the agent loop.
- README and architecture documentation updated for 0.1.11-era features.

### Changed

- **Path validation**: new module and error handling so skill paths stay within allowed roots (prevents traversal and out-of-workspace access).
- Task progress reporting and tool execution context improved; task completion guidelines and execution feedback refined.
- Release workflow: Cargo.lock verification with clearer error messaging; dependency and reqwest configuration updates.

---

## [0.1.11] - 2026-03-07

### Added

- `urlencoding` dependency and can-do query handler for skill discovery/query.
- Changelog (internal) now records only modified files based on snapshot comparison.

### Changed

- Bump Python SDK and crate versions to 0.1.11; Cargo.lock updated.

### Fixed

- Changelog snapshot logic and dependency alignment for release consistency.

---

## [0.1.10] - 2026-03-05

### Added

- Windows sandbox: extended job object limits for better resource and process handling.

### Changed

- Version bump to v0.1.10 across the project.

### Fixed

- Windows WSL2 sandbox path handling and release tag consistency.

---

## [0.1.9] - 2026-03-05

### Added

- Evotown: evolution testing platform section in README; evolution effect validation documented.
- **Sandbox (Linux)**: whitelist-only read-only bind of minimal `/etc` paths (ld.so, resolv.conf, hosts, ssl/certs) for runtime; no passwd/shadow exposed. Seccomp hardening and bash runtime with skill-local `node_modules/.bin` on PATH (no host paths).
- New skill "find-skills" added to the skilllite manifest.
- **Security**: pre-compiled regex patterns for faster static scanning; scan cache concurrency safety improved.
- Agent: user message handling with context appending; input processing to prevent context overflow; task hint management and filtering; task completion auto-marking; skill evolution and workspace handling; evolution handling refactored and integrated with skilllite-evolution.
- Root Cargo.lock committed for reproducible CI `--locked` builds.

### Security

- Linux sandbox follows **minimum exposure**: only the `/etc` entries required for dynamic linking and TLS are mounted read-only; sensitive system files remain outside the container. Bash/Node runtimes use only skill-local `node_modules`, not host paths.

### Changed

- Output directory resolution and rule filtering; documentation and .gitignore organization.
- Release workflow: reference root Cargo.lock and streamline paths.
- Dependency audit: refactored metadata handling (behavior unchanged); command structure and dependencies refactored.
- Skill directory discovery and YAML handling; swarm: peer matching, capability inference, single-task execution and routing, Ctrl+C handling.

### Fixed

- Benchmark scripts: correct build output paths for workspace layout.
- Release workflow: fix CI build failure when Cargo.lock was missing at repo root.

---

## [0.1.8] - 2026-02-17

Versions 0.1.1–0.1.7 were not tagged; 0.1.8 is the next release after 0.1.0.

### Added

- `find_sandbox_binary` for more reliable sandbox discovery in SDK and CLI.
- **RPC-based skill management and IPC executor**: skill execution moved behind a single binary interface; multi-script tool handling improved. Decouples SDK from implementation and keeps the security boundary inside the Rust process.
- Optional LLM-based dependency resolution at init (opt-in); CLI: init, quickstart, MCP server for IDE integration; **dependency audit** for supply-chain vulnerability scanning (PyPI/OSV).
- Agent: chat feature, memory support, anti-hallucination measures, streaming output; plan management and textification; transcript management and compaction.
- Cursor IDE integration command; quickstart for onboarding; preview server for HTML/PPT; **path validation** (restrict skill access to allowed dirs, prevent traversal); planning rules configuration.
- Observability and logging; long-text summarization env config; skill path validation.

### Architecture

- **Execution boundary**: SDK invokes the sandbox binary via subprocess; all skill runs and sandbox logic live in one process. Enables future MCP/HTTP servers and other clients without duplicating security code.

### Changed

- **Rename**: SkillBox → SkillLite; related configs, docs, and security benchmark references updated. Same security model; clearer product identity.
- GitHub Actions: enhanced workflow and caching for Rust project; Cargo.lock added for dependency management; reqwest default features tuned.
- Skill metadata and tool definitions; deprecated audit/security event logging code paths removed (install-time scan and runtime sandbox unchanged).
- SDK invokes the sandbox binary for execution; agent RPC environment preparation and result handling improved; task plan persistence.

### Fixed

- Output handling in demos and agent RPC environment preparation; CLI input and security scan error management.

---

## [0.1.0] - 2026-02-01

### Added

- Initial public release: secure skill execution engine with OS-native sandbox (Seatbelt on macOS, bwrap/seccomp on Linux).
- CLI: skill management, chat, run, scan, init, MCP server; Python SDK with thin bridge; MCP server for IDE integration.
- **Security (full-chain defense)**: install-time static scan, pre-execution confirmation, and runtime isolation with resource limits — three layers so untrusted skills cannot escape or exfiltrate.

### Architecture

- **Thin SDK + single binary**: Python SDK is a thin bridge; all skill execution and sandboxing run in the Rust binary. Clear boundary keeps the security core in one place and allows any language/framework to integrate via CLI or MCP.

### Changed

- GitHub Actions: write permissions for release creation.

### Notes

- This release established the stable baseline for all later versions.

---

## Links

[Unreleased]: https://github.com/EXboys/skilllite/compare/v0.1.19...HEAD
[0.1.19]: https://github.com/EXboys/skilllite/releases/tag/v0.1.19
[0.1.16]: https://github.com/EXboys/skilllite/releases/tag/v0.1.16
[0.1.15]: https://github.com/EXboys/skilllite/releases/tag/v0.1.15
[0.1.14]: https://github.com/EXboys/skilllite/releases/tag/v0.1.14
[0.1.13]: https://github.com/EXboys/skilllite/releases/tag/v0.1.13
[0.1.12]: https://github.com/EXboys/skilllite/releases/tag/v0.1.12
[0.1.11]: https://github.com/EXboys/skilllite/releases/tag/v0.1.11
[0.1.10]: https://github.com/EXboys/skilllite/releases/tag/v0.1.10
[0.1.9]: https://github.com/EXboys/skilllite/releases/tag/v0.1.9
[0.1.8]: https://github.com/EXboys/skilllite/releases/tag/v0.1.8
[0.1.0]: https://github.com/EXboys/skilllite/releases/tag/v0.1.0
