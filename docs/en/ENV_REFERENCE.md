# SkillLite Environment Variables Reference

This document lists all environment variables supported by SkillLite, including default values, type descriptions, and usage scenarios.

- **Quick Start**: Only `BASE_URL`, `API_KEY`, and `MODEL` are required to run
- **Full Template**: See [.env.example.full](../../.env.example.full)

---

## Recommended Variables & Aliases

Prefer `SKILLLITE_*` as primary variables; compatible with `OPENAI_*`, `BASE_URL` and other industry-standard names; `SKILLBOX_*`, `AGENTSKILL_*` are deprecated — please migrate.

| Recommended | Aliases (fallback order) | Description |
|-------------|--------------------------|-------------|
| `SKILLLITE_API_BASE` | `OPENAI_API_BASE`, `OPENAI_BASE_URL`, `BASE_URL` | LLM API endpoint |
| `SKILLLITE_API_KEY` | `OPENAI_API_KEY`, `API_KEY` | API key |
| `SKILLLITE_MODEL` | `OPENAI_MODEL`, `MODEL` | Model name |
| `SKILLLITE_AUDIT_LOG` | (legacy: `SKILLBOX_AUDIT_LOG`) | Audit log path |
| `SKILLLITE_QUIET` | (legacy: `SKILLBOX_QUIET`) | Quiet mode |
| `SKILLLITE_CACHE_DIR` | (legacy: `SKILLBOX_CACHE_DIR`, `AGENTSKILL_CACHE_DIR`) | Skill env cache directory |

**Deprecation**: `SKILLBOX_*` and `AGENTSKILL_*` will be removed in a future major version. Please migrate to the corresponding `SKILLLITE_*` variables.

---

## Config Source Priority

When the same variable is set in multiple places, resolution order is (highest → lowest):

| Priority | Source | Description |
|----------|--------|--------------|
| 1 | **CLI / explicit args** | Command-line args (e.g. `--message`), quickstart prompts, desktop settings overrides |
| 2 | **Environment variables** | `export VAR=value` set before process start |
| 3 | **.env file** | `.env` in workspace or current dir; `load_dotenv` loads and **does not overwrite** existing env |
| 4 | **Defaults** | Code fallbacks (e.g. `LlmConfig::from_env()` defaults) |

**Example**: If `.env` has `MODEL=deepseek-chat` but the user selects `gpt-4` in the desktop UI, `gpt-4` wins (CLI/explicit > .env).

### UI locale (chat & scheduled jobs)

| Variable | Values | Default | Description |
|----------|--------|---------|-------------|
| `SKILLLITE_UI_LOCALE` | `zh`, `en` | unset = no extra block | The desktop app sets this from **Settings → UI language** for child processes; it is merged into the system prompt append segment so the model defaults to that language for explanations. **`skilllite schedule tick` and interactive chat** behave consistently when set. CLI users may `export SKILLLITE_UI_LOCALE=en`. |

---

## Layered by Scenario

| Tier | Count | Description |
|------|-------|--------------|
| **Required** | 3 | `BASE_URL`, `API_KEY`, `MODEL` (or `SKILLLITE_*` equivalents) |
| **Common** | 5–8 | `SKILLS_DIR`, `ALLOW_NETWORK`, `EXECUTION_TIMEOUT`, `SKILLLITE_SANDBOX_LEVEL`, `ENABLE_SANDBOX` |
| **Advanced** | 15–20 | Long text, planning, audit, resource limits; configure as needed |
| **Internal** | rest | Subprocess/sandbox internal; usually no user config needed |

- **`.env.example`**: Required + Common only
- **`.env.example.full`**: Full variable list with tier comments

---

## LLM API Configuration <small>[Required]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_API_BASE` | string | - | **Recommended**. LLM API endpoint; aliases: `OPENAI_API_BASE`, `OPENAI_BASE_URL`, `BASE_URL` |
| `SKILLLITE_API_KEY` | string | - | **Recommended**. API key; aliases: `OPENAI_API_KEY`, `API_KEY` |
| `SKILLLITE_MODEL` | string | `deepseek-chat` | **Recommended**. Model name; aliases: `OPENAI_MODEL`, `MODEL` |
| `SKILLLITE_MAX_TOKENS` | int | `8192` | Max output tokens per LLM call; higher reduces write_output truncation (some APIs like Claude support more) |

**Usage**: Required for all LLM calls. Supports any OpenAI-compatible API provider (DeepSeek, Qwen, Ollama, etc.). If you see `Recovered truncated JSON for write_output`, try increasing `SKILLLITE_MAX_TOKENS`.

---

## Skills & Output <small>[Common]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLS_DIR` | string | `./skills` | Skills directory path, supports relative/absolute paths (compatible with `./.skills`) |
| `SKILLLITE_SKILLS_DIR` | string | - | Same as above (alias) |
| `SKILLLITE_SKILLS_REPO` | string | `EXboys/skilllite` | GitHub repo for `skilllite init` to download skills when `skills/` is empty (e.g. `owner/repo`) |
| `SKILLLITE_OUTPUT_DIR` | string | `{workspace}/output` | Output directory (`write_output`, screenshots, etc.). When unset, `workspace` matches `SKILLLITE_WORKSPACE`, else **current working directory** (desktop chat child uses the selected project root as cwd). |
| (internal) | string | Current working directory | Root for skill paths in sandbox; legacy `SKILLBOX_SKILLS_ROOT` (no SKILLLITE name yet) |

---

## Scheduled runs `schedule tick` <small>[Optional]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_SCHEDULE_ENABLED` | bool | **Treated as `false` when unset** | **Required `1`/`true` for `skilllite schedule tick` when it would call the LLM**; if unset, due jobs are skipped with a hint. **`--dry-run` does not require this variable**. |

**Usage**: When `.skilllite/schedule.json` exists and cron invokes `tick`, set `SKILLLITE_SCHEDULE_ENABLED=1` in the crontab environment or `.env` so scheduled runs are explicitly opt-in.

---

## Network Configuration <small>[Common]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `ALLOW_NETWORK` | bool | `False` | Whether to allow Skills to access the network |
| (internal) | bool | - | Same as above (internal; legacy `SKILLBOX_ALLOW_NETWORK`) |
| `NETWORK_TIMEOUT` | int | `30` | Network request timeout (seconds) |

**Usage**: Set `ALLOW_NETWORK=True` when using Skills that require network access (e.g. weather, HTTP requests).

---

## P2P Swarm `skilllite swarm` <small>[Optional]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_SWARM_URL` | string | unset (daemon sets `http://127.0.0.1:<port>` if unset) | Base URL for `delegate_to_swarm` (e.g. `http://192.168.1.10:7700` for a remote node). |
| `SKILLLITE_SWARM_TOKEN` | string | unset | When non-empty, the swarm HTTP API requires `Authorization: Bearer <token>` on every request (`/task`, `/status`, `/can-do`). All nodes and clients (including `delegate_to_swarm`) must use the **same** value. **Recommended** whenever using `--listen 0.0.0.0:*`. |

**CLI default listen address** is `127.0.0.1:7700` (loopback). Use `--listen 0.0.0.0:7700` only when other hosts must connect; pair with `SKILLLITE_SWARM_TOKEN` in production-like setups.

---

## Run-scoped artifact HTTP `skilllite artifact-serve` <small>[Optional]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_ARTIFACT_SERVE_ALLOW` | string | unset | Must be **`1`** for the **`skilllite artifact-serve`** CLI subcommand to **bind** and listen. Without it, the command exits with an error (subcommand stays in the default binary, but no socket is opened). **Embedders** calling `skilllite_artifact::run_artifact_http_server` directly are **not** gated by this variable. |
| `SKILLLITE_ARTIFACT_HTTP_SERVE` | string | unset | *(Tests / tooling)* When set to a `skilllite` executable path, Python SDK integration tests spawn `artifact-serve` from that binary. |

**Implementation note (not an env var)**: The reference HTTP router (`skilllite_artifact::artifact_router` / `skilllite artifact-serve`) applies Axum **`DefaultBodyLimit`** of **64 MiB** per `PUT`. Oversized bodies receive HTTP **413**; constant `skilllite_artifact::MAX_ARTIFACT_BODY_BYTES`.

---

## Sandbox & Security <small>[Common]</small>

Sandbox-related variables are read through the **config layer** (`SandboxEnvConfig::from_env()`); config accepts `SKILLLITE_*` (recommended) and legacy `SKILLBOX_*`.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_SANDBOX_LEVEL` | int | `3` | **Recommended**. Sandbox level (1/2/3) |
| `SKILLLITE_NO_SANDBOX` | bool | `false` | Disable sandbox (not recommended) |
| `SKILLLITE_ALLOW_LINUX_NAMESPACE_FALLBACK` | bool | `false` | **Linux only**. If bubblewrap/firejail are missing or fail, allow a weak fallback using PID/UTS/network namespaces only (**no** bwrap-style filesystem sandbox). Default `false` refuses execution (fail-closed, aligned with Windows). Legacy: `SKILLBOX_ALLOW_LINUX_NAMESPACE_FALLBACK` |
| `SKILLLITE_ALLOW_PLAYWRIGHT` | bool | `false` | Relax sandbox for Playwright Skills |
| `SKILLLITE_AUTO_APPROVE` | bool | `false` | **Recommended**. Auto-approve L3 prompts (not recommended) |
| `SKILLLITE_SCRIPT_ARGS` | string | - | Extra args passed to scripts |
| `ENABLE_SANDBOX` | bool | `true` | Whether to enable sandbox |
| `SANDBOX_BUILTIN_TOOLS` | bool | `false` | Run read_file/write_file in subprocess for isolation |
| `SKILLLITE_TRUST_BYPASS_CONFIRM` | bool | `false` | Allow execution of Community/Unknown trust tier skills without confirmation (CLI/Python only; MCP uses `confirmed` param) |

**Sandbox Level Description**:

| Level | Description |
|-------|-------------|
| 1 | No sandbox, full trust |
| 2 | Sandbox + allow .env/git/venv/cache/Playwright (permissive) |
| 3 | Scan + confirm, after confirmation same as L2 (default) |

**Usage**:
- When sandbox is unavailable: `SKILLLITE_SANDBOX_LEVEL=1` or `SKILLLITE_NO_SANDBOX=1` (no isolation)
- On Linux without bubblewrap but you still want **limited** isolation: `SKILLLITE_ALLOW_LINUX_NAMESPACE_FALLBACK=1` (weak; use with care)
- When Skill is stuck: `SKILLLITE_LOG_LEVEL=debug` to view progress

**Skill pre-spawn static scan (implementation note)**: One unified precheck (`SKILL.md` supply-chain heuristics + entry `ScriptScanner`) lives in `skilllite-sandbox` as `run_skill_precheck` / `SkillPrecheckSummary`. It runs **before skill code executes for sandbox levels 1, 2, and 3** inside the CLI/`skilllite exec` runner (with `SKILLLITE_AUTO_APPROVE` / TTY consent when the report is non-empty). The agent chat path runs the same function for **all levels** via `EventSink`, then passes `SandboxRunOptions.skip_skill_precheck` so the runner does not repeat it (avoids non-TTY false blocks). MCP `run_skill` at Level 3 uses the same precheck and returns `scan_kind: l3_skill_precheck` (legacy JSON label) + `scan_id` when review is needed; at Level 1–2 the runner runs the precheck unless skipped. **`run_command`** additionally runs `scan_shell_command` (shell-oriented `ScriptScanner` stages: entropy, base64 heuristics, download/decode/execute chain) before spawning `sh -c`; findings require confirmation (`confirm_required`).

---

## Resource Limits <small>[Advanced]</small>

Sandbox resource limits are read through **config** (`SandboxEnvConfig`); legacy `SKILLBOX_*` is still accepted.

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_TIMEOUT_SECS` | int | `30` | **Recommended**. Sandbox execution timeout (seconds) |
| `SKILLLITE_MAX_MEMORY_MB` | int | `256` | **Recommended**. Sandbox max memory (MB) |
| `EXECUTION_TIMEOUT` | int | `120` | Single execution timeout (seconds) |
| `MAX_MEMORY_MB` | int | `256` | Maximum memory (MB) |

**Usage**: For Skills with many dependencies (e.g. xiaohongshu-writer), consider `EXECUTION_TIMEOUT=300`.

---

## Long Text & Summarization <small>[Advanced]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_CHUNK_SIZE` | int | `6000` | Chunk size (~1.5k tokens/chunk) |
| `SKILLLITE_HEAD_CHUNKS` | int | `3` | Number of head chunks for head-tail summary |
| `SKILLLITE_TAIL_CHUNKS` | int | `3` | Number of tail chunks for head-tail summary |
| `SKILLLITE_MAX_OUTPUT_CHARS` | int | `8000` | Max output length for summary (~2k tokens) |
| `SKILLLITE_SUMMARIZE_THRESHOLD` | int | `15000` | Use summary when exceeding this length, otherwise truncate |
| `SKILLLITE_TOOL_RESULT_MAX_CHARS` | int | `8000` | Max characters for single tool result in Agent loop |
| `SKILLLITE_READ_FILE_TOOL_RESULT_MAX_CHARS` | int | `786432` | `read_file` only: max bytes before head+tail truncation when sending tool result to the model (default ~768 KiB) |

**Usage**: Adjust as needed for very long context; usually no modification required.

---

## Session & Compaction <small>[Advanced]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_COMPACTION_THRESHOLD` | int | `16` | Compact conversation history when message count exceeds this (~8 turns) |
| `SKILLLITE_COMPACTION_KEEP_RECENT` | int | `10` | Number of recent messages to keep after compaction |
| `SKILLLITE_CONTEXT_SOFT_LIMIT_CHARS` | int | `320000` | Before each agent LLM call, if estimated history + user payload (chars) exceeds this, shrink tool results and may run compaction even below the message-count threshold. `0` disables. Roughly ~80k tokens at ~4 chars/token |
| `SKILLLITE_MEMORY_FLUSH_ENABLED` | bool | `true` | Enable pre-compaction memory flush (OpenClaw-style) |
| `SKILLLITE_MEMORY_FLUSH_THRESHOLD` | int | `12` | Trigger memory flush at this message count (lower = more frequent) |

**Usage**: Lower `COMPACTION_THRESHOLD` (e.g. `12`) for more frequent compaction; raise it if compaction triggers too often. The `/compact` command manually triggers compaction regardless of threshold. If you still hit provider input-token limits with few messages but large tool output, rely on `CONTEXT_SOFT_LIMIT_CHARS` (lower the value to shrink earlier) or lower `SKILLLITE_READ_FILE_TOOL_RESULT_MAX_CHARS` / `SKILLLITE_TOOL_RESULT_MAX_CHARS`.

**Memory auto-flush**: When `enable_memory` is on, a silent turn runs at `MEMORY_FLUSH_THRESHOLD` (default 12 messages, ~6 turns) to prompt the model to write durable memories to `memory/YYYY-MM-DD.md`. Lower `MEMORY_FLUSH_THRESHOLD` (e.g. `8` or `6`) for more frequent memory triggers.

---

## Planning & Rules <small>[Advanced]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_COMPACT_PLANNING` | bool | auto | 1=compact, 0=full. When unset: only claude/gpt-4/gpt-5/gemini-2 use compact; deepseek/qwen/7b/ollama get full |

Planning rules are defined in `planning_rules.rs`; no external JSON config needed.

---

## Evolution Engine <small>[Advanced]</small>

**Common variables** (most use cases only need these):

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_EVOLUTION` | string | `1` | Evolution mode: `1`/`true` all enabled, `0`/`false` disabled, `prompts`/`memory`/`skills` for specific dimensions only |
| `SKILLLITE_MAX_EVOLUTIONS_PER_DAY` | int | `20` | Daily evolution cap (counts `evolution_log` rows `evolution_run` **and** `evolution_run_noop`; scheduler-only `evolution_run_outcome` rows do not count) |
| `SKILLLITE_EVOLUTION_INTERVAL_SECS` | int | `600` | **A9** Periodic trigger interval (seconds). Default 10 min; each tick may spawn `evolution run` when growth scheduling says “due” |
| `SKILLLITE_EVOLUTION_DECISION_THRESHOLD` | int | `10` | **A9** OR-trigger: when raw unprocessed decision rows (`evolved = 0`, any tool count) ≥ this value, growth is due |
| `SKILLLITE_EVO_TRIGGER_WEIGHTED_MIN` | int | `3` | **A9** Weighted sum over the latest `SKILLLITE_EVO_TRIGGER_SIGNAL_WINDOW` meaningful unprocessed decisions (`total_tools ≥ 1`); weight 2 if `feedback = neg` or `failed_tools > 0`, else 1. Growth is due when sum ≥ this |
| `SKILLLITE_EVO_TRIGGER_SIGNAL_WINDOW` | int | `10` | **A9** How many latest meaningful unprocessed decisions participate in the weighted sum |
| `SKILLLITE_EVO_SWEEP_INTERVAL_SECS` | int | `86400` | **A9** If no **material** `evolution_run` log (`type = evolution_run`, not `evolution_run_noop`) for this many seconds and weighted sum ≥ 1, growth is due (low-priority catch-up) |
| `SKILLLITE_EVO_MIN_RUN_GAP_SEC` | int | `0` | **A9** Minimum seconds since last **material** `evolution_run` before another autorun; `0` disables (`evolution_run_noop` does not satisfy the gap) |
| `SKILLLITE_EVO_SHALLOW_PREFLIGHT` | bool | `1` | **Run** When `1`, skip snapshot + learners if weighted/unprocessed backlog is empty and skills dir / external learning do not require work (reduces periodic **NoOp** cost; may defer one tick of prompt **rule retirement**). Set `0` to disable |
| `SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS` | int | `10` | Minimum count of stable successful unprocessed decisions before **active** evolution proposals are built (separate from A9 growth spawn) |
| `SKILLLITE_EVOLUTION_SNAPSHOT_KEEP` | int | `10` | Max number of evolution txn snapshot dirs under `chat/prompts/_versions/` (oldest removed first by directory name). **`0` = never prune** — keeps full local prompt history without Git; disk use grows with runs |
| `SKILLLITE_EVO_AUTO_EXECUTE_LOW_RISK` | bool | `1` | When policy runtime is enabled, allow coordinator to auto-execute low-risk proposals |
| `SKILLLITE_EVO_POLICY_RUNTIME_ENABLED` | bool | `1` | Enable coordinator policy runtime; decision is evaluated as `allow` / `ask` / `deny` with an auditable reason chain |
| `SKILLLITE_EVO_DENY_CRITICAL` | bool | `1` | Deny critical-risk proposals by default in policy runtime (`policy_denied` backlog status) |
| `SKILLLITE_EVO_RISK_BUDGET_LOW_PER_DAY` | int | `5` | Daily auto-execution budget for low-risk proposals (`0` = never auto execute) |
| `SKILLLITE_EVO_RISK_BUDGET_MEDIUM_PER_DAY` | int | `0` | Daily auto-execution budget for medium-risk proposals (`0` = manual queue only) |
| `SKILLLITE_EVO_RISK_BUDGET_HIGH_PER_DAY` | int | `0` | Daily auto-execution budget for high-risk proposals (`0` = manual queue only) |
| `SKILLLITE_EVO_RISK_BUDGET_CRITICAL_PER_DAY` | int | `0` | Daily auto-execution budget for critical-risk proposals (`0` = deny/queue per policy) |
| `SKILLLITE_EVO_PROFILE` | string | (unset) | Evolution trigger profile: `demo` = more frequent (demos/testing), `default` or unset = same as original defaults, `conservative` = less frequent (production/cost-saving). **Unset or `default` keeps behavior unchanged.** |
| `SKILLLITE_EVO_ACCEPTANCE_WINDOW_DAYS` | int | `3` | Acceptance linkage window size (days) for backlog auto-judgement |
| `SKILLLITE_EVO_ACCEPTANCE_MIN_SUCCESS_RATE` | float | `0.70` | Acceptance threshold: minimum `first_success_rate` within the window |
| `SKILLLITE_EVO_ACCEPTANCE_MAX_CORRECTION_RATE` | float | `0.20` | Acceptance threshold: maximum `user_correction_rate` within the window |
| `SKILLLITE_EVO_ACCEPTANCE_MAX_ROLLBACK_RATE` | float | `0.20` | Acceptance threshold: maximum rollback rate (`auto_rollback / evolution_run`) in the window |
| `SKILLLITE_SKILL_DEDUP_DESCRIPTION` | string | `1` | Skill same-round dedup: `0` disables description similarity check; otherwise skips if new skill's description is highly similar to existing pending |

**Advanced variables** (fine-tune thresholds when needed; when unset, values come from `SKILLLITE_EVO_PROFILE` or defaults):

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_EVO_COOLDOWN_HOURS` | float | `1` | Cooldown (hours) after last **material** evolution (`evolution_run`); `evolution_run_noop` does not reset this clock |
| `SKILLLITE_EVO_RECENT_DAYS` | int | `7` | Time window (days) for decision statistics |
| `SKILLLITE_EVO_RECENT_LIMIT` | int | `100` | Max number of decisions to consider in the window |
| `SKILLLITE_EVO_MEANINGFUL_MIN_TOOLS` | int | `2` | Min tool calls per decision to count as "meaningful" |
| `SKILLLITE_EVO_MEANINGFUL_THRESHOLD_SKILLS` | int | `3` | Skills evolution: trigger when meaningful ≥ this and (failures > 0 or repeated patterns) |
| `SKILLLITE_EVO_MEANINGFUL_THRESHOLD_MEMORY` | int | `3` | Memory evolution: trigger when meaningful ≥ this |
| `SKILLLITE_EVO_MEANINGFUL_THRESHOLD_PROMPTS` | int | `5` | Prompts evolution: trigger when meaningful ≥ this and (failures or replans meet min) |
| `SKILLLITE_EVO_FAILURES_MIN_PROMPTS` | int | `2` | Prompts evolution: min failures to consider |
| `SKILLLITE_EVO_REPLANS_MIN_PROMPTS` | int | `2` | Prompts evolution: min replans to consider |
| `SKILLLITE_EVO_REPEATED_PATTERN_MIN_COUNT` | int | `3` | Repeated pattern: same pattern count ≥ this and success rate met |
| `SKILLLITE_EVO_REPEATED_PATTERN_MIN_SUCCESS_RATE` | float | `0.8` | Repeated pattern: min success rate (0–1) |

**Learner input windows** (tune for higher recall of rules/examples/memory/skills; defaults match historical behavior):

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_EVO_PROMPT_EXAMPLE_MIN_TOOLS` | int | `2` | Prompt evolution: min `total_tools` for **example** generation candidates; `1` for lighter tasks, `3` for stricter signal |
| `SKILLLITE_EVO_PROMPT_RULE_SUMMARY_LIMIT` | int | `10` | Prompt evolution: max recent decisions per success/fail bucket for **rule extraction** summaries |
| `SKILLLITE_EVO_MEMORY_RECENT_DAYS` | int | `7` | Memory evolution: decision query lookback (days) |
| `SKILLLITE_EVO_MEMORY_DECISION_LIMIT` | int | `15` | Memory evolution: max rows from that window in the extraction prompt |
| `SKILLLITE_EVO_SKILL_QUERY_RECENT_DAYS` | int | `7` | Skill synth SQL: lookback days for pattern/failure queries |
| `SKILLLITE_EVO_SKILL_QUERY_DECISION_LIMIT` | int | `100` | Skill synth SQL: max rows scanned in that window |
| `SKILLLITE_EVO_SKILL_FAILURE_SAMPLE_LIMIT` | int | `5` | Max failure-context rows sampled per skill |

**Extra evolution audit event types**: `evolution_run_scope` (scope JSON before full learners), `evolution_shallow_skip` (shallow preflight skip), `rule_extraction_parse_failed` (rule JSON parse failure), `evolution_run_noop` (execution finished with no changelog rows — timeline + daily cap; passive cooldown still uses **material** `evolution_run` only).

**Evolution triggers (A9)**: Growth scheduling (`skilllite-evolution::growth_schedule`) marks a run **due** when **any** of: **periodic** interval elapsed (`SKILLLITE_EVOLUTION_INTERVAL_SECS`, default 10 min), **weighted signals** over a sliding window (≥ `SKILLLITE_EVO_TRIGGER_WEIGHTED_MIN`, default 3), **raw backlog** (unprocessed rows ≥ `SKILLLITE_EVOLUTION_DECISION_THRESHOLD`, default 10), or **sweep** (long idle since last **material** `evolution_run` + weighted ≥ 1). **`SKILLLITE_EVO_MIN_RUN_GAP_SEC`** can throttle consecutive autoruns (also material-run keyed). **`ChatSession`** (`skilllite chat` / `agent-rpc` subprocess) runs timers in-process; **SkillLite Assistant** spawns `skilllite evolution run` from **Life Pulse** with merged workspace + UI env. In-chat **P7 “authorize evolution” bubbles** after partial_success/failure are **not** shown; scheduling aligns with the evolution panel, not inline chat prompts.

**“Run outcome” is not “evolution failed”**: A9 **due** only means a check fired; **whether proposals are built** depends on passive/active thresholds, cooldown, daily cap, etc. When **only the periodic arm** is due and **no proposals would be built**, the agent timer and desktop Life Pulse **skip** that tick (no subprocess, no `evolution_run_outcome` spam). **Signal or sweep arms** still attempt `evolution run`; if proposals are still empty, logs include a more specific reason code. Older rows may still show the generic message.

**More aggressive evolution (quick presets)**: `SKILLLITE_EVO_PROFILE=demo`, or keep default and lower `SKILLLITE_EVO_COOLDOWN_HOURS` (e.g. `0.25`), `SKILLLITE_EVO_ACTIVE_MIN_STABLE_DECISIONS` (e.g. `4`), and `SKILLLITE_EVO_MEANINGFUL_THRESHOLD_*` as needed. Run more **tool-heavy successful** turns to refill `evolved=0` stable decisions for **active** proposals.

**SkillLite Assistant (desktop)**: **Run now** on a backlog row in the evolution detail view uses the same **Settings** API key, model, and base URL as chat. **Life Pulse** merges workspace `.env` with the same **Settings** snapshot pushed from the UI into the child environment for `skilllite evolution run` and `skilllite schedule tick`, matching the chat subprocess rules—you usually **do not** need a project `.env` API key for background evolution. For pure CLI use outside the app, keep using `.env` or shell env as before.

Under **Settings → Evolution**, in-app overrides let you set check interval, decision-count threshold, `SKILLLITE_EVO_PROFILE`, and `SKILLLITE_EVO_COOLDOWN_HOURS` with the same merge rules; **non-empty values win over workspace `.env`**. Leave fields empty to keep `.env` / built-in defaults (apply with Save).

**Same-round skill dedup**: A single evolution run runs failure-driven generation first, then success-driven generation; both may write new skills to `_pending`. To avoid duplicates, before writing: ① skip if same name already exists in pending; ② skip if description is highly similar (normalized descriptions are substrings of each other). Set `SKILLLITE_SKILL_DEDUP_DESCRIPTION=0` to disable the description check.

**Skill generation failure**: If you see `Failed to parse skill generation JSON: EOF`, the LLM output was likely truncated. Try increasing `SKILLLITE_MAX_TOKENS` (e.g. 16384) and retry.

**Skills needing review (L4 failed)**: Network-request skills may be saved as draft when L4 security scan fails. Run `skilllite evolution status` to see `(needs review)`. Add `compatibility: Requires Python 3.x, network access` to SKILL.md front matter, then run `skilllite evolution confirm <name>`.

---

## Observability & Audit <small>[Advanced]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_AUDIT_LOG` | string | `{data_root}/audit` | Audit dir or file. Dir → daily `audit_YYYY-MM-DD.jsonl`; `.jsonl` suffix → single file |
| `SKILLLITE_AUDIT_DISABLED` | bool | `false` | Set to `1` to disable audit (enabled by default) |
| `SKILLLITE_AUDIT_CONTEXT` | string | `cli` | Audit context (e.g. session_id, invoker); also written to **Agent-layer edit events** (`edit_applied` / `edit_previewed` / `edit_failed` / `edit_inserted`) as `context`, same as `skill_invocation` |
| `SKILLLITE_SECURITY_EVENTS_LOG` | string | - | Security events log (intercepts, scan_high, etc.) |
| `SKILLLITE_SUPPLY_CHAIN_BLOCK` | bool | `false` | P0 observable vs P1 block: `1` blocks on HashChanged/SignatureInvalid/TrustDeny; `0` (default) only shows status |
| `SKILLLITE_LOG_LEVEL` | string | `info` | Rust log level (**recommended**) |
| `SKILLLITE_LOG_JSON` | bool | `false` | Output JSON logs |
| `SKILLLITE_SKILL_DENYLIST` | string | - | **P1 manual deny**: comma-separated SKILL `name` values (same as audit `skill_id`), merged with denylist files below; if matched, `run` / `exec` / `bash` / Agent / MCP refuse before execution |
| `SKILLLITE_AUDIT_ALERT_WEBHOOK` | string | - | With `skilllite audit-report --alert`, POST JSON alerts to this URL (or use `--webhook`) in addition to stderr and tracing |
| `SKILLLITE_AUDIT_ALERT_MAX_INVOCATIONS_PER_SKILL` | int | `200` | Alert: `skill_invocation` count for one skill exceeds this in the window |
| `SKILLLITE_AUDIT_ALERT_MIN_INVOCATIONS_FOR_FAILURE` | int | `5` | Alert: minimum invocations before failure-rate rule applies |
| `SKILLLITE_AUDIT_ALERT_FAILURE_RATIO` | float | `0.5` | Alert: failure rate ≥ this (0–1) with invocations ≥ previous row |
| `SKILLLITE_AUDIT_ALERT_EDIT_UNIQUE_PATHS` | int | `80` | Alert: distinct paths in `edit_*` events exceed this in the window |

**P1 denylist files** (merged with `SKILLLITE_SKILL_DENYLIST`, one `name` per line, `#` comments): `~/.skilllite/skill-denylist.txt`, `{data_root}/.skilllite/skill-denylist.txt`, and `./.skilllite/skill-denylist.txt` from the current working directory. **Unblock**: remove the name from those files or from the env var (re-read on each execution; no process restart required).

**P1 audit analysis**: `skilllite audit-report [--dir DIR] [--hours N] [--json] [--alert] [--webhook URL]` — aggregates `audit_*.jsonl` for per-skill invocation counts, failure rates, and `edit_*` path distribution; `--alert` emits to stderr and tracing (target `skilllite::audit`), optionally POSTs to the webhook.

**Edit audit (Agent built-in tools)**: `search_replace`, `preview_edit`, and `insert_lines` append JSONL lines with events such as `edit_applied`, `edit_previewed`, `edit_failed`, and `edit_inserted`. Each record includes `edit_id` (UUID), top-level `path`, `workspace`, and on failure `reason` / `tool`; each write is followed by `flush` for streaming consumers.

**`skill_invocation` summaries**: `input_summary` / `output_summary` `preview` fields are redacted before write (same family of rules as Agent tools: common JSON keys, `KEY=value` lines, long `sk-…` / `Bearer …` tokens). `preview_redacted` is `true` when any redaction applied to the full string (not only the preview window). `len` remains the original byte length.

**Development & tests**: `skilllite-agent` builtin unit tests set `SKILLLITE_AUDIT_DISABLED=1` at process start so tests do not pollute the default audit path. For other crates or integration tests, set `SKILLLITE_AUDIT_DISABLED=1` explicitly if needed.

---

## A11 High-Risk Tool Confirmation <small>[Advanced]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_HIGH_RISK_CONFIRM` | string | `write_key_path,run_command` | Comma-separated: high-risk ops requiring confirmation. **Default omits `network`** (network-enabled skills no longer prompt separately; add `network` to the list or use `all` to restore the extra network gate). Sensitive reads via `run_command` (e.g. `cat .env`) always require confirmation and are **not** skipped when `run_command` is omitted or set to `none`. **`none` does not bypass the `run_command` blocked (L0) tier**: machine-wide catastrophic patterns are rejected **before** spawn regardless of this variable (e.g. fork bomb, `rm -rf /`, `rm -rf /*`, `sudo rm -rf /`, `dd` to block devices, `mkfs.*`). Desktop **auto-approve** only applies to `risk_tier: low` on `confirmation_request` events. |

---

## Debug & Advanced <small>[Advanced/Internal]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_DEBUG` | bool | `false` | Set to `1` to print sandbox debug info (legacy: `SKILLBOX_DEBUG`) |
| `SKILLLITE_USE_IPC` | bool | auto | Whether to use IPC mode (usually faster); legacy: `SKILLBOX_USE_IPC` |
| `SKILLLITE_PATH` | string | - | skilllite binary path |
| `SKILLLITE_CACHE_DIR` | string | `{cache}/skilllite/envs` | Skill env cache (Python venv / Node); `skilllite env clean` |
| `SKILLLITE_IPC_POOL_SIZE` | int | `10` | IPC connection pool size (legacy: `SKILLBOX_IPC_POOL_SIZE`) |
| `MCP_SANDBOX_TIMEOUT` | int | `30` | MCP sandbox timeout (seconds) |

---

## Recommended Configurations by Scenario

### Quick Start (Minimal Config)

```bash
BASE_URL=https://api.deepseek.com/v1
API_KEY=your_key
MODEL=deepseek-chat
```

### Network-enabled Skills (e.g. weather, HTTP)

```bash
# Add to minimal config
ALLOW_NETWORK=True
```

### Skills with Many Dependencies (e.g. xiaohongshu-writer)

```bash
# Add to common config
EXECUTION_TIMEOUT=300
```

### Sandbox Unavailable / Debugging

```bash
SKILLLITE_SANDBOX_LEVEL=1
# or
SKILLLITE_LOG_LEVEL=debug
```

### Production Audit

```bash
# Audit is enabled by default, stored daily at ~/.skilllite/audit/audit_YYYY-MM-DD.jsonl
# Custom directory (also daily):
SKILLLITE_AUDIT_LOG=/var/log/skilllite/audit
# Or single file (no daily rotation):
SKILLLITE_AUDIT_LOG=/var/log/skilllite/audit.jsonl

SKILLLITE_SECURITY_EVENTS_LOG=~/.skilllite/audit/security.jsonl
```
