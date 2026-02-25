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
| `SKILLLITE_AUDIT_LOG` | `SKILLBOX_AUDIT_LOG` | Audit log path |
| `SKILLLITE_QUIET` | `SKILLBOX_QUIET` | Quiet mode |
| `SKILLLITE_CACHE_DIR` | `SKILLBOX_CACHE_DIR`, `AGENTSKILL_CACHE_DIR` | Skill env cache directory |

**Deprecation**: `SKILLBOX_*` and `AGENTSKILL_*` will be removed in a future major version. Please migrate to the corresponding `SKILLLITE_*` variables.

---

## Layered by Scenario

| Tier | Count | Description |
|------|-------|--------------|
| **Required** | 3 | `BASE_URL`, `API_KEY`, `MODEL` (or `SKILLLITE_*` equivalents) |
| **Common** | 5–8 | `SKILLS_DIR`, `ALLOW_NETWORK`, `EXECUTION_TIMEOUT`, `SKILLBOX_SANDBOX_LEVEL`, `ENABLE_SANDBOX` |
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
| `SKILLS_DIR` | string | `./.skills` | Skills directory path, supports relative/absolute paths |
| `SKILLLITE_SKILLS_DIR` | string | - | Same as above (alias) |
| `SKILLLITE_SKILLS_REPO` | string | `EXboys/skilllite` | GitHub repo for `skilllite init` to download skills when `.skills/` is empty (e.g. `owner/repo`) |
| `SKILLLITE_OUTPUT_DIR` | string | `{workspace_root}/output` | Output directory for reports, images, etc. |
| `SKILLBOX_SKILLS_ROOT` | string | Current working directory | Root for skill paths in sandbox (**deprecated**, no SKILLLITE replacement yet) |

---

## Network Configuration <small>[Common]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `ALLOW_NETWORK` | bool | `False` | Whether to allow Skills to access the network |
| `SKILLBOX_ALLOW_NETWORK` | bool | - | Same as above (internal use, **deprecated**) |
| `NETWORK_TIMEOUT` | int | `30` | Network request timeout (seconds) |

**Usage**: Set `ALLOW_NETWORK=True` when using Skills that require network access (e.g. weather, HTTP requests).

---

## Sandbox & Security <small>[Common]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `ENABLE_SANDBOX` | bool | `true` | Whether to enable sandbox |
| `SKILLBOX_SANDBOX_LEVEL` | int | `3` | Sandbox level (see table below) |
| `SKILLBOX_ALLOW_PLAYWRIGHT` | bool | `false` | Skip sandbox for Skills using Playwright |
| `SANDBOX_BUILTIN_TOOLS` | bool | `false` | Run read_file/write_file in subprocess for isolation |
| `SKILLBOX_AUTO_APPROVE` | bool | `false` | Auto-approve L3 security prompts (not recommended) |
| `SKILLLITE_TRUST_BYPASS_CONFIRM` | bool | `false` | Allow execution of Community/Unknown trust tier skills without confirmation (CLI/Python only; MCP uses `confirmed` param) |

**Sandbox Level Description**:

| Level | Description |
|-------|-------------|
| 1 | No sandbox, full trust |
| 2 | Sandbox + allow .env/git/venv/cache/Playwright (permissive) |
| 3 | Scan + confirm, after confirmation same as L2 (default) |

**Usage**:
- When sandbox is unavailable: `SKILLBOX_SANDBOX_LEVEL=1`
- When Skill is stuck: `SKILLBOX_DEBUG=1` to view progress

---

## Resource Limits <small>[Advanced]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `EXECUTION_TIMEOUT` | int | `120` | Single execution timeout (seconds) |
| `SKILLBOX_TIMEOUT_SECS` | int | - | Same as above (alias) |
| `MAX_MEMORY_MB` | int | `256` | Maximum memory (MB) |
| `SKILLBOX_MAX_MEMORY_MB` | int | - | Same as above (alias) |

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

**Usage**: Adjust as needed for very long context; usually no modification required.

---

## Session & Compaction <small>[Advanced]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_COMPACTION_THRESHOLD` | int | `16` | Compact conversation history when message count exceeds this (~8 turns) |
| `SKILLLITE_COMPACTION_KEEP_RECENT` | int | `10` | Number of recent messages to keep after compaction |
| `SKILLLITE_MEMORY_FLUSH_ENABLED` | bool | `true` | Enable pre-compaction memory flush (OpenClaw-style) |
| `SKILLLITE_MEMORY_FLUSH_THRESHOLD` | int | `12` | Trigger memory flush at this message count (lower = more frequent) |

**Usage**: Lower `COMPACTION_THRESHOLD` (e.g. `12`) for more frequent compaction; raise it if compaction triggers too often. The `/compact` command manually triggers compaction regardless of threshold.

**Memory auto-flush**: When `enable_memory` is on, a silent turn runs at `MEMORY_FLUSH_THRESHOLD` (default 12 messages, ~6 turns) to prompt the model to write durable memories to `memory/YYYY-MM-DD.md`. Lower `MEMORY_FLUSH_THRESHOLD` (e.g. `8` or `6`) for more frequent memory triggers.

---

## Planning & Rules <small>[Advanced]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_COMPACT_PLANNING` | bool | auto | 1=compact, 0=full. When unset: only claude/gpt-4/gpt-5/gemini-2 use compact; deepseek/qwen/7b/ollama get full |

Planning rules are defined in `planning_rules.rs`; no external JSON config needed.

---

## Observability & Audit <small>[Advanced]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_AUDIT_LOG` | string | - | Audit log path (confirm→execute→command) |
| `SKILLBOX_AUDIT_LOG` | string | - | Same as above (**deprecated**, use `SKILLLITE_AUDIT_LOG`) |
| `SKILLLITE_SECURITY_EVENTS_LOG` | string | - | Security events log (intercepts, scan_high, etc.) |
| `SKILLLITE_LOG_LEVEL` | string | `info` | Rust log level (**recommended**); alias `SKILLBOX_LOG_LEVEL` (**deprecated**) |
| `SKILLLITE_LOG_JSON` | bool | `false` | Output JSON logs; alias `SKILLBOX_LOG_JSON` (**deprecated**) |

---

## Debug & Advanced <small>[Advanced/Internal]</small>

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLBOX_DEBUG` | bool | `false` | Set to `1` to print sandbox debug info |
| `SKILLBOX_USE_IPC` | bool | auto | Whether to use IPC mode (usually faster) |
| `SKILLLITE_PATH` | string | - | skilllite binary path |
| `SKILLBOX_BINARY_PATH` | string | - | Same as above (**deprecated**, use `SKILLLITE_PATH`) |
| `SKILLBOX_CACHE_DIR` | string | - | Sandbox cache (**deprecated**, use `SKILLLITE_CACHE_DIR`) |
| `SKILLLITE_CACHE_DIR` | string | `{cache}/skilllite/envs` | Skill env cache (Python venv / Node); `skilllite env clean`; aliases `SKILLBOX_CACHE_DIR`, `AGENTSKILL_CACHE_DIR` (**deprecated**) |
| `SKILLBOX_IPC_POOL_SIZE` | int | `10` | IPC connection pool size |
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
SKILLBOX_SANDBOX_LEVEL=1
# or
SKILLBOX_DEBUG=1
```

### Production Audit

```bash
SKILLLITE_AUDIT_LOG=~/.skilllite/audit/audit.jsonl
SKILLLITE_SECURITY_EVENTS_LOG=~/.skilllite/audit/security.jsonl
```
