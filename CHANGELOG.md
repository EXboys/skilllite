# Changelog

All notable changes to SkillLite are documented in this file. This log emphasizes **technical impact** (what problems we solve), **security** (defense-in-depth and safe evolution), and **architecture** (layering, boundaries, and evolvability). For full design rationale see [Architecture](docs/en/ARCHITECTURE.md) .

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Documentation

- **Onboarding / mental model**: Added [Pick your path](docs/en/START_PATHS.md) / [选择你的路径](docs/zh/START_PATHS.md) (desktop vs sandbox/MCP vs full stack). Root `README.md` now surfaces a short triage table and links the page from the documentation strip and the docs index.
- **README / 中文 README**: Removed the **framework-by-framework self-evolving agent** comparison table (hard to keep fair as the market moves). Replaced with a short **“evolution without lowering the safety bar”** positioning. Expanded the **Desktop Assistant** `<details>` block with installers, `START_PATHS`, in-app **Settings** tabs, and a screenshot (`docs/images/assistant-settings-model-api.png`).

### Changed

- **Evolution defaults**: `EvolutionThresholds` passive cooldown (`SKILLLITE_EVO_COOLDOWN_HOURS` when unset) is now **0.5** hours (30 minutes) instead of **1** hour, so default installs retry passive proposals sooner while staying less aggressive than `SKILLLITE_EVO_PROFILE=demo` (**0.25** h).
- **Artifact HTTP** (`skilllite_artifact::run_artifact_http_server`, including `skilllite artifact-serve`): Refuses to bind on a **non-loopback** address when no bearer token is configured, unless **`SKILLLITE_ARTIFACT_HTTP_ALLOW_INSECURE_NO_AUTH=1`** (logs a loud warning). Loopback binds without a token still start but emit a **warning**. Optional **`SKILLLITE_ARTIFACT_HTTP_REQUIRE_AUTH=1`** requires a non-empty token even on loopback. Empty `--token` values are treated as unset.

---

## [0.1.26] - 2026-04-14

### Changed

- **SkillLite Assistant (evolution panel)**: `skilllite_load_evolution_status` now returns structured **A9** (`inspect_growth_due`) and **passive** (`passive_schedule_diagnostics`) snapshots plus `last_material_run_ts`, `would_have_evolution_proposals`, and `empty_proposals_reason` (desktop Life Pulse periodic anchor wired in). Detail tab shows a **Schedule diagnostics** block (EN/ZH i18n).
- **Evolution audit / passive cooldown**: When a full execution finishes **without** changelog material, `evolution_log` now records `evolution_run_noop` (timeline + `SKILLLITE_MAX_EVOLUTIONS_PER_DAY`). **Passive cooldown**, **A9 sweep**, and **`SKILLLITE_EVO_MIN_RUN_GAP_SEC`** still use the latest **material** `evolution_run` only, so repeated no-output runs no longer push the passive clock forward.
- **Evolution (A9)**: When **only the periodic** growth arm is due and **no proposals** would be built, **ChatSession** and **SkillLite Assistant Life Pulse** now **skip** spawning `run_evolution` / `skilllite evolution run`, avoiding noisy `evolution_run_outcome` rows. Signal/sweep arms unchanged. Empty-proposal audit reasons are **more specific** (disabled / daily cap / cooldown / passive+active idle); EN/ZH evolution panel copy maps the new strings.
- **Evolution learners (recall + observability)**: New env-tuned windows — `SKILLLITE_EVO_PROMPT_EXAMPLE_MIN_TOOLS` (default **`2`**, higher recall than the legacy hardcoded **`3`** for example candidates), `SKILLLITE_EVO_PROMPT_RULE_SUMMARY_LIMIT`, `SKILLLITE_EVO_MEMORY_RECENT_DAYS`, `SKILLLITE_EVO_MEMORY_DECISION_LIMIT`, `SKILLLITE_EVO_SKILL_QUERY_RECENT_DAYS`, `SKILLLITE_EVO_SKILL_QUERY_DECISION_LIMIT`, `SKILLLITE_EVO_SKILL_FAILURE_SAMPLE_LIMIT` (remaining defaults match prior hardcoded behavior). Audit log adds **`evolution_run_scope`** (JSON scope per txn), **`evolution_shallow_skip`**, and documents **`rule_extraction_parse_failed`** in EN/ZH `ENV_REFERENCE.md`; assistant i18n labels the new types.

### Fixed

- **Agent (RPC / desktop chat)**: After a **streamed** LLM completion, `emit_assistant_visible` (e.g. **tool-result summary** when the model did not write a long user-facing reply) was routed through `on_text`, which **dropped** the payload as a presumed duplicate of streamed chunks. Users saw tool output in the timeline but **no final assistant `text` event**. `RpcEventSink` and `TerminalEventSink` now override `emit_assistant_visible` to **always emit**, clearing the streaming dedupe flag first.
- **SkillLite Assistant (desktop)**: **「猜你想问」** follow-up suggestions now render **at the bottom of the chat transcript** (inside the scroll area) instead of a fixed strip between the message list and the composer, so they no longer shrink the reading viewport. Suggestion generation **locks output language to the last user message** (CJK vs English) so Chinese input is not paired with English-only recommendations when the model previously followed assistant-side English.
- **Agent (task planning)**: After a successful tool batch, when the model answered in prose but had not yet called `complete_task`, the planner soft-nudge path no longer emits that prose to the UI immediately. This avoids a duplicate assistant bubble when the next turn streams the final user-facing summary.
- **Agent (task planning)**: When all planned tasks complete in the same tool batch, the loop used to treat `assistant_content` as “insubstantial” whenever tool-call turns suppressed free-form assistant text—so it almost always scheduled an extra **closing** LLM turn. That second turn produced a second user-visible summary (often redundant or garbled). The loop now treats **trailing tool results** as substantial when long enough (e.g. weather payloads) and **skips** the closing round in that case. When skipping, the agent **emits one `emit_assistant_visible`** line derived from that tool payload so the desktop/RPC UI shows an assistant bubble (not only the tool chip). `build_agent_result` / transcript use the same fallback when there is no non-empty assistant message.
- **Agent (task planning)**: Tool-derived user summaries **exclude** `complete_task` JSON acks (they often appear *after* skills like `weather` and are >80 chars). Picking those acks produced useless assistant text; the picker now prefers earlier substantive tool bodies.

### Release

- Workspace, Python SDK, and SkillLite Assistant (Tauri) bumped to **0.1.26**.

---

## [0.1.25] - 2026-04-12

### Fixed

- **SkillLite Assistant (desktop / CI)**: Dropped the **`remark-gfm`** dependency so production bundles cannot include **`mdast-util-gfm-autolink-literal`** (RegExp with `(?<=` + Unicode properties breaks older **WKWebView**). Options are typed from **footnote / table / strikethrough** packages only. **release-desktop** runs **`scripts/verify-no-lookbehind-in-dist.mjs`** after every Tauri build.

### Release

- Workspace, Python SDK, and SkillLite Assistant (Tauri) bumped to **0.1.25**.

---

## [0.1.24] - 2026-04-12

### Release

- **Version-only bump** to **0.1.24**: PyPI (and similar indexes) do not allow replacing already-uploaded artifacts for **0.1.23**; publishing fixed wheels/sdist requires a new version number. Workspace, Python SDK, and SkillLite Assistant (Tauri) pins aligned to **0.1.24**.

---

## [0.1.23] - 2026-04-12

### Fixed

- **SkillLite Assistant (desktop / onboarding)**: First-run health check reported **「模型来源 / 尚未填写 API Key」** even when the user had entered a key. The webview invoked `skilllite_health_check` with **`api_key`**, but Tauri maps Rust `api_key` ↔ **`apiKey`** in JSON; the key was dropped server-side. The UI now sends **`apiKey`**; the command deserializes **`apiKey`** (and accepts legacy **`api_key`** via serde alias).

### Release

- Workspace, Python SDK, and SkillLite Assistant (Tauri) bumped to **0.1.23**; internal crate path dependency pins aligned to **0.1.23**.

---

## [0.1.22] - 2026-04-08

### Added

- **`skilllite-artifact` crate**: Consolidates `LocalDirArtifactStore`, Axum HTTP router, blocking HTTP client, and `run_artifact_http_server` behind features `local` / `server` / `client`. Python SDK adds stdlib `artifact_put` / `artifact_get` plus integration tests; see `docs/openapi/artifact-store-http-v1.yaml`. The crate exposes `skilllite_artifact::Error` / `skilllite_artifact::Result` (with `Other(#[from] anyhow::Error)`) per workspace Rust conventions; `HttpArtifactStore::try_new` now returns `skilllite_artifact::Result<Self>` (**replaces** the previous `BuildError` type). Reference HTTP `PUT` requests are limited to **64 MiB** per body (`DefaultBodyLimit`; HTTP **413** when exceeded; `MAX_ARTIFACT_BODY_BYTES`).

### Changed

- **SkillLite Assistant (desktop)**: IDE **workspace file tree** lists via `skilllite_list_workspace_entries` now returns **`truncated`**, **`max_entries`**, and **`max_depth`** (walk caps unchanged). UI adds **skeleton + indexing hint**, **non-blocking refresh** (new request supersedes stale results), **`useTransition` / `useDeferredValue`** to keep the main thread responsive on large listings, a **truncation notice** when the backend reports caps, and an **entry count** footer.
- **CLI `artifact-serve`**: Still compiled in the default `skilllite` binary (`artifact_http` remains a default feature), but the command **refuses to bind** unless **`SKILLLITE_ARTIFACT_SERVE_ALLOW=1`** is set. Reduces accidental network exposure; embedders calling `skilllite_artifact::run_artifact_http_server` directly are unchanged.

### Release

- Workspace, Python SDK, and SkillLite Assistant (Tauri) bumped to **0.1.22**; internal crate path dependency pins aligned to **0.1.22**.

---

## [0.1.21] - 2026-04-05

### Changed

- **Evolution memory rollup**: After Memory 进化写入按月分卷（`memory/evolution/<dim>/YYYY-MM.md`），引擎会为该月重算 **`YYYY-MM.rollup.md` 去重汇总**（确定性合并同键条目；原始按次记录保留在分卷中）。各维索引表增加「去重汇总」列；抽取时的已有知识摘要**优先纳入**最新 rollup 尾部以降低重复抽取。
- **Evolution shallow preflight**: Before snapshots/learners, `run_evolution` may **early-exit** when there is **no** unprocessed decision backlog, **zero** weighted signals, **no** skill tree work, and **external learning** is off (`SKILLLITE_EVO_SHALLOW_PREFLIGHT`, default **on**). Trade-off: **prompt rule retirement** may skip that tick; set `SKILLLITE_EVO_SHALLOW_PREFLIGHT=0` to disable. Manual `skilllite evolution run` remains **forced** and bypasses this gate.
- **A9 evolution growth schedule**: Centralized in `skilllite-evolution::growth_schedule` — default **periodic interval 600s (10 min)**; **weighted** trigger over the latest meaningful unprocessed decisions (default sum ≥ **3**, window **10**; failures / negative user feedback count double); **OR** raw unprocessed row count ≥ `SKILLLITE_EVOLUTION_DECISION_THRESHOLD` (default **10**); optional **sweep** after `SKILLLITE_EVO_SWEEP_INTERVAL_SECS` (default 24h) when weighted ≥ 1; optional **`SKILLLITE_EVO_MIN_RUN_GAP_SEC`** between `evolution_run` logs. **Active** proposal minimum stable successes use **`SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS`** (default **10**), decoupled from A9 spawn threshold. Desktop Life Pulse, evolution status API, and `ChatSession` periodic / turn triggers use the same rules (merged env + UI overrides where applicable).
- **SkillLite Assistant (desktop)**: Default window size is now **1280×800** (was 900×650) so the three-column layout and settings drawer fit comfortably; window remains **resizable** with **minimum 1024×600** for small screens.
- **SkillLite Assistant (desktop)**: **Settings → Evolution** tab holds **in-app overrides** for `SKILLLITE_EVOLUTION_INTERVAL_SECS`, `SKILLLITE_EVOLUTION_DECISION_THRESHOLD`, `SKILLLITE_EVO_PROFILE` (demo / conservative), and `SKILLLITE_EVO_COOLDOWN_HOURS`, merged into chat / Life Pulse / manual evolution-run env (same as LLM overrides). Status API and Life Pulse **growth due** use the effective values. `ENV_REFERENCE` documents the behavior.
- **Evolution memory knowledge**: Auto-extracted knowledge is written under `memory/evolution/` as **five dimensions** (`entities/`, `relations/`, `episodes/`, `preferences/`, `patterns/`) with **monthly shards** (`YYYY-MM.md`), plus per-dimension index files (`entities.md`, …) and a root **`INDEX.md`**. Legacy single-file `knowledge.md` is still read for dedup summaries and can remain on disk. Evolution **snapshots/restore** now copy the whole `memory/evolution/` tree (with **legacy** restore from snapshot `memory/knowledge.md` when present). **`index_evolution_knowledge`** reindexes all evolution markdown except navigational indexes and clears stale `evolution/*` FTS rows first.
- **SkillLite Assistant (desktop)**: Chat **internal steps** timeline **auto-expands** when it contains a pending **execution confirmation** or **clarification** (so users do not have to discover the collapsed block). When still collapsed, a **“action needed”** badge is shown. The input area adds an optional **auto-allow execution confirmations** checkbox (persisted in app settings); it only affects `confirmation` prompts, not clarification option pickers.
- **SkillLite Assistant (desktop)**: **Settings** opens as a **right-edge drawer** beside the status column (parallel to chat; no full-window dimmer). The header **Settings** control **toggles** the drawer; **Escape** or **Close** still dismisses it. Tabs, fields, save/cancel, and persistence are unchanged.

### Release

- Workspace, Python SDK, and SkillLite Assistant (Tauri) bumped to **0.1.21**; internal crate path dependency pins aligned to **0.1.21**.

---

## [0.1.20] - 2026-04-02

### Added

- **Evolution audit**: `evolution_log` / `evolution.log` now records **`evolution_run_outcome`** when a `run_evolution` invocation ends as **`SkippedBusy`**, **`NoScope`** (no proposals or coordinator mutex busy), or **`Err`** (failure reason), so desktop/agent triggers are traceable without relying on tracing only.

### Changed

- **SkillLite Assistant (desktop)**: Evolution backlog list in **运行与队列** hides rows that are **fully done** (`status=executed` with `acceptance_status` **`met`** or **`not_met`**); rows still in the acceptance window (**`pending_validation`**) remain visible. CLI `skilllite evolution backlog` is unchanged (full table; use `--status` to filter).
- **Evolution coordinator**: **Shadow mode** (`SKILLLITE_EVO_SHADOW_MODE`) is **removed**; proposals no longer stop at a shadow-only backlog state. When **`SKILLLITE_EVO_POLICY_RUNTIME_ENABLED=0`**, the coordinator **executes** the selected proposal directly (except **critical** still respects **`SKILLLITE_EVO_DENY_CRITICAL`**). **Low-risk auto-execute** remains **on** by default (`SKILLLITE_EVO_AUTO_EXECUTE_LOW_RISK`) when policy runtime is enabled.
- **A9**: `ChatSession` again runs **in-process** periodic + decision-count evolution. **Life Pulse** still spawns `skilllite evolution run` on the same interval/threshold (workspace `.env`). **Desktop chat** no longer injects **P7 in-conversation “evolution_options”** bubbles on tool partial_success/failure or `complete_task` partial_success/failure (`useChatEvents.ts`); use the **自进化** panel or wait for A9.

### Release

- Workspace, Python SDK, and SkillLite Assistant (Tauri) bumped to **0.1.20**; internal crate path dependency pins aligned to **0.1.20**.

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

[Unreleased]: https://github.com/EXboys/skilllite/compare/v0.1.26...HEAD
[0.1.26]: https://github.com/EXboys/skilllite/releases/tag/v0.1.26
[0.1.25]: https://github.com/EXboys/skilllite/releases/tag/v0.1.25
[0.1.24]: https://github.com/EXboys/skilllite/releases/tag/v0.1.24
[0.1.23]: https://github.com/EXboys/skilllite/releases/tag/v0.1.23
[0.1.22]: https://github.com/EXboys/skilllite/releases/tag/v0.1.22
[0.1.21]: https://github.com/EXboys/skilllite/releases/tag/v0.1.21
[0.1.20]: https://github.com/EXboys/skilllite/releases/tag/v0.1.20
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
