# SkillLite Environment Variables Reference

This document lists all environment variables supported by SkillLite, including default values, type descriptions, and usage scenarios.

- **Quick Start**: Only `BASE_URL`, `API_KEY`, and `MODEL` are required to run
- **Full Template**: See [.env.example.full](../../.env.example.full)

---

## LLM API Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `BASE_URL` | string | - | **Required**. LLM API endpoint, e.g. `https://api.deepseek.com/v1` |
| `API_KEY` | string | - | **Required**. LLM API key |
| `MODEL` | string | `deepseek-chat` | Model name |

**Usage**: Required for all LLM calls. Supports any OpenAI-compatible API provider (DeepSeek, Qwen, Ollama, etc.).

---

## Skills & Output

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLS_DIR` | string | `./.skills` | Skills directory path, supports relative/absolute paths |
| `SKILLLITE_SKILLS_DIR` | string | - | Same as above (alias) |
| `SKILLLITE_OUTPUT_DIR` | string | `{workspace_root}/output` | Output directory for reports, images, etc. |
| `SKILLBOX_SKILLS_ROOT` | string | Current working directory | Root directory for skill paths in sandbox; skill_dir must be under it |

---

## Network Configuration

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `ALLOW_NETWORK` | bool | `False` | Whether to allow Skills to access the network |
| `SKILLBOX_ALLOW_NETWORK` | bool | - | Same as above (used internally by sandbox) |
| `NETWORK_TIMEOUT` | int | `30` | Network request timeout (seconds) |

**Usage**: Set `ALLOW_NETWORK=True` when using Skills that require network access (e.g. weather, HTTP requests).

---

## Sandbox & Security

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `ENABLE_SANDBOX` | bool | `true` | Whether to enable sandbox |
| `SKILLBOX_SANDBOX_LEVEL` | int | `3` | Sandbox level (see table below) |
| `SKILLBOX_ALLOW_PLAYWRIGHT` | bool | `false` | Skip sandbox for Skills using Playwright |
| `SANDBOX_BUILTIN_TOOLS` | bool | `false` | Run read_file/write_file in subprocess for isolation |
| `SKILLBOX_AUTO_APPROVE` | bool | `false` | Auto-approve L3 security prompts (not recommended) |

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

## Resource Limits

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `EXECUTION_TIMEOUT` | int | `120` | Single execution timeout (seconds) |
| `SKILLBOX_TIMEOUT_SECS` | int | - | Same as above (alias) |
| `MAX_MEMORY_MB` | int | `512` | Maximum memory (MB) |
| `SKILLBOX_MAX_MEMORY_MB` | int | - | Same as above (alias) |

**Usage**: For Skills with many dependencies (e.g. xiaohongshu-writer), consider `EXECUTION_TIMEOUT=300`.

---

## Long Text & Summarization

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

## Planning & Rules

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_PLANNING_RULES_PATH` | string | - | Custom path for planning_rules.json |

---

## Observability & Audit

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLLITE_AUDIT_LOG` | string | - | Audit log path (confirm→execute→command) |
| `SKILLBOX_AUDIT_LOG` | string | - | Same as above (alias) |
| `SKILLLITE_SECURITY_EVENTS_LOG` | string | - | Security events log (intercepts, scan_high, etc.) |
| `SKILLBOX_LOG_LEVEL` | string | `info` | Rust log level: trace\|debug\|info\|warn\|error |
| `SKILLBOX_LOG_JSON` | bool | `false` | Whether to output JSON format logs |

---

## Debug & Advanced

| Variable | Type | Default | Description |
|----------|------|---------|-------------|
| `SKILLBOX_DEBUG` | bool | `false` | Set to `1` to print sandbox debug info |
| `SKILLBOX_USE_IPC` | bool | auto | Whether to use IPC mode (usually faster) |
| `SKILLLITE_PATH` | string | - | skilllite binary path |
| `SKILLBOX_BINARY_PATH` | string | - | Same as above (alias) |
| `SKILLBOX_CACHE_DIR` | string | - | Sandbox cache directory |
| `SKILLLITE_CACHE_DIR` | string | `{cache}/skilllite/envs` | Skill environment cache (Python venv / Node); `skilllite env clean` cleans this dir; fallback: `AGENTSKILL_CACHE_DIR` |
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
