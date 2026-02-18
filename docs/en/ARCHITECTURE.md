# SkillLite Project Architecture

> This document records the core architecture, design philosophy, and key implementation details of the SkillLite project.

## Overview

**SkillLite** is a lightweight AI Agent Skills execution engine with the following core features:

- **Built-in Native System-Level Sandbox**: Rust-implemented native system-level security isolation
- **Zero Dependencies**: Single binary, millisecond cold start
- **Local Execution**: Code and data never leave your machine
- **LLM Agnostic**: Compatible with all OpenAI API format LLM providers

### Tech Stack

| Component | Technology |
|-----------|------------|
| Sandbox Executor | Rust (skilllite binary) |
| Python SDK | Python 3.x (python-sdk) |
| macOS Sandbox | Seatbelt (sandbox-exec) |
| Linux Sandbox | Namespace + Seccomp |

## Project Structure

```
skillLite/
├── skilllite/                  # Rust sandbox executor (core)
│   └── src/
│       ├── main.rs             # CLI entry (chat/add/list/mcp/run/exec)
│       ├── sandbox/            # Sandbox implementation
│       │   ├── executor.rs     # Sandbox executor and security levels
│       │   ├── macos.rs        # macOS Seatbelt sandbox
│       │   ├── linux.rs        # Linux Namespace sandbox
│       │   └── security/       # Security scanning module
│       └── skill/              # Skill metadata parsing
│
├── python-sdk/                 # Python SDK
│   └── skilllite/
│       ├── api.py              # chat, run_skill, scan_code, execute_code
│       ├── binary.py           # Binary management
│       ├── cli.py              # CLI entry (forwards to binary)
│       └── ipc.py              # IPC client
│
├── langchain-skilllite/        # LangChain adapter (separate package)
├── benchmark/                  # Performance tests
└── .skills/                    # Skills directory (examples)
```

## Core Modules

### 1. Sandbox Security Levels

```rust
pub enum SandboxLevel {
    Level1,  // No sandbox - direct execution
    Level2,  // Sandbox isolation only (macOS Seatbelt / Linux namespace + seccomp)
    Level3,  // Sandbox isolation + static code scanning (default)
}
```

### 2. Resource Limits

```rust
pub struct ResourceLimits {
    pub max_memory_mb: u64,   // Default 512MB
    pub timeout_secs: u64,    // Default 30 seconds
}
```

**Environment Variables:**
- `SKILLBOX_SANDBOX_LEVEL`: Sandbox level (1/2/3)
- `SKILLBOX_MAX_MEMORY_MB`: Maximum memory limit
- `SKILLBOX_TIMEOUT_SECS`: Execution timeout
- `SKILLBOX_AUTO_APPROVE`: Auto-approve dangerous operations

### 3. Security Features

| Capability | Description |
|------------|-------------|
| Process Isolation | Each Skill runs in independent process |
| Filesystem Isolation | Only Skill directory and temp directory accessible |
| Network Isolation | Disabled by default, can be enabled on demand |
| Resource Limits | CPU, memory, execution time limits |

### 4. SKILL.md Metadata

```yaml
---
name: my-skill
description: A skill that does something useful.
compatibility: Requires Python 3.x with requests library, network access
license: MIT
metadata:
  author: example-org
  version: "1.0"
---
```

**Auto-detection from `compatibility` field:**
- Network access: keywords like "network", "internet", "http", "api", "web"
- Language: "Python", "Node", "JavaScript", "bash", "shell"
- Dependencies: Known packages like requests, pandas, axios, etc.

## Execution Flow

```
User Input
    ↓
chat() or skilllite chat
    ↓
Rust agent loop (skilllite binary)
    ↓
┌─────────────────────────────────────┐
│ 1. Generate system prompt           │
│ 2. Call LLM                         │
│ 3. Parse tool calls                 │
│ 4. Execute tools (run/exec)          │
│ 5. Return results to LLM            │
│ 6. Repeat until complete            │
└─────────────────────────────────────┘
    ↓
skilllite run/exec
    ↓
┌─────────────────────────────────────┐
│ Rust Sandbox:                       │
│ 1. Parse SKILL.md metadata          │
│ 2. Setup virtual environment        │
│ 3. Level 3: Static code scanning    │
│ 4. Level 2+: Start sandbox          │
│ 5. Execute script                   │
│ 6. Monitor resource usage           │
│ 7. Return result                    │
└─────────────────────────────────────┘
```

## CLI Commands

```bash
skilllite run <skill_dir> '<input_json>'      # Run Skill
skilllite exec <skill_dir> <script> '<json>'  # Execute script directly
skilllite scan <skill_dir>                   # Scan Skill
skilllite validate <skill_dir>               # Validate Skill
skilllite security-scan <script_path>        # Security scan
skilllite chat                               # Interactive agent chat
skilllite add owner/repo                     # Add skills from GitHub
skilllite list                               # List installed skills
```

---

*For detailed Chinese documentation, see [中文版](../zh/ARCHITECTURE.md)*

