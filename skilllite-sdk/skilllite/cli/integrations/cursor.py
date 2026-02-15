"""
Cursor IDE integration for skilllite CLI.

Provides the init-cursor command to set up SkillLite integration with Cursor IDE.
"""

import argparse
import json
import os
import sys
from pathlib import Path
from typing import List, Dict, Any

from ...sandbox.skillbox import (
    install,
    is_installed,
    get_installed_version,
)
from .opencode import _get_available_skills, _detect_best_command


def _generate_cursor_rules_content(skills: List[Dict[str, Any]]) -> str:
    """Generate the .cursor/rules/skilllite.mdc content for Cursor integration."""
    skills_list = ""
    if skills:
        for skill in skills:
            skills_list += f"- **{skill['name']}**: {skill['description']}\n"
    else:
        skills_list = "- (No pre-defined skills found. Use skilllite_execute_code for code execution.)\n"

    return f'''---
description: SkillLite - Execute code or pre-defined skills in a secure sandbox
globs:
alwaysApply: false
---

## Overview

SkillLite provides a secure sandbox execution environment via MCP. Code runs isolated in a system-level sandbox (macOS Seatbelt / Linux Namespace), preventing malicious code from affecting the host.

## When to use SkillLite vs terminal

| Scenario | Terminal | SkillLite |
|----------|---------|-----------|
| git operations | ✅ | |
| Read/write project files | ✅ | |
| Execute user-provided code | | ✅ |
| Network/API calls | | ✅ |
| Data analysis | | ✅ |
| Run untrusted scripts | | ✅ |
| Execute potentially dangerous commands | | ✅ |

## Available MCP Tools

### 1. skilllite_execute_code
Execute arbitrary code (Python/JavaScript/Bash) in sandbox.

**Params:**
- `language`: "python" | "javascript" | "bash"
- `code`: Code to execute
- `confirmed`: Whether to confirm execution (required for high-risk code)
- `scan_id`: Scan ID (required when confirming)

**Security confirmation flow:**
When dangerous code is detected, returns security report and `scan_id`. After showing user the issues, if user agrees, call again with `confirmed=true` and the returned `scan_id`.

### 2. skilllite_run_skill
Execute a pre-defined skill.

**Params:**
- `skill_name`: Skill name
- `input`: Skill input params (JSON object)

### 3. skilllite_list_skills
List all available pre-defined skills. No params.

### 4. skilllite_get_skill_info
Get detailed info for a skill, including input schema.

**Params:**
- `skill_name`: Skill name

### 5. skilllite_scan_code
Scan code safety only, no execution. For pre-checking safety.

**Params:**
- `language`: "python" | "javascript" | "bash"
- `code`: Code to scan

## Pre-defined Skills

{skills_list}

## Examples

### Execute Python code
```
skilllite_execute_code(language="python", code="print(sum(range(1, 101)))")
```

### Handle dangerous code
1. Call `skilllite_execute_code` to execute
2. If returns `requires_confirmation=true`, show user the security issues
3. After user confirms, call again with `confirmed=true` and `scan_id`

### Use pre-defined skills
```
skilllite_list_skills()  # List available skills
skilllite_get_skill_info(skill_name="calculator")  # Get skill params
skilllite_run_skill(skill_name="calculator", input={{"operation": "add", "a": 5, "b": 3}})
```
'''


def _generate_cursor_mcp_config(command: List[str], skills_dir: str) -> Dict[str, Any]:
    """Generate Cursor MCP configuration (.cursor/mcp.json)."""
    # Cursor format: command is the executable, args is the rest
    cmd_executable = command[0]
    cmd_args = command[1:] if len(command) > 1 else []

    return {
        "mcpServers": {
            "skilllite": {
                "command": cmd_executable,
                "args": cmd_args,
                "env": {
                    "SKILLBOX_SANDBOX_LEVEL": "3",
                    "SKILLLITE_SKILLS_DIR": skills_dir
                }
            }
        }
    }


def _get_cursor_global_config_path() -> Path:
    """Get the global Cursor MCP config path (~/.cursor/mcp.json)."""
    return Path.home() / ".cursor" / "mcp.json"


def cmd_init_cursor(args: argparse.Namespace) -> int:
    """Initialize Cursor IDE integration.

    Supports two modes:
    - Project-level (default): writes to <project>/.cursor/mcp.json
      Only available in the current Cursor project.
    - Global (--global): writes to ~/.cursor/mcp.json
      Available in ALL Cursor projects.
    """
    try:
        is_global = getattr(args, "global_mode", False)
        project_dir = Path(args.project_dir or os.getcwd())
        skills_dir = args.skills_dir or "./.skills"

        mode_label = "Global" if is_global else "Project"
        print(f"🚀 Initializing SkillLite integration for Cursor IDE ({mode_label})...")
        if not is_global:
            print(f"   Project directory: {project_dir}")
        print()

        # 1. Check if skillbox is installed
        if not is_installed():
            print("⚠ skillbox not installed. Installing...")
            install(show_progress=True)
        else:
            version = get_installed_version()
            print(f"✓ skillbox installed (v{version})")

        # 2. Detect best command to start MCP server
        command, command_desc = _detect_best_command()
        print(f"✓ MCP command: {command_desc}")
        print(f"   → {' '.join(command)}")

        # 3. Determine config paths based on mode
        if is_global:
            # Global: ~/.cursor/mcp.json
            mcp_config_path = _get_cursor_global_config_path()
            cursor_dir = mcp_config_path.parent
            # For global config, skills_dir must be absolute
            skills_dir_for_config = str(Path(project_dir / (skills_dir[2:] if skills_dir.startswith("./") else skills_dir)).resolve())
        else:
            # Project-level: <project>/.cursor/mcp.json
            cursor_dir = project_dir / ".cursor"
            mcp_config_path = cursor_dir / "mcp.json"
            skills_dir_for_config = skills_dir

        cursor_dir.mkdir(parents=True, exist_ok=True)

        # 4. Generate and write MCP config
        config = _generate_cursor_mcp_config(command, skills_dir_for_config)

        if mcp_config_path.exists() and not args.force:
            # Merge with existing config
            try:
                existing = json.loads(mcp_config_path.read_text())
                if "mcpServers" not in existing:
                    existing["mcpServers"] = {}
                existing["mcpServers"]["skilllite"] = config["mcpServers"]["skilllite"]
                config = existing
                print(f"✓ Updated existing {mcp_config_path}")
            except Exception:
                print(f"⚠ Could not parse existing {mcp_config_path}, overwriting")
        else:
            print(f"✓ Created {mcp_config_path}")

        mcp_config_path.write_text(json.dumps(config, indent=2, ensure_ascii=False))

        # 5. Get available skills
        skills_dir_clean = skills_dir[2:] if skills_dir.startswith("./") else skills_dir
        full_skills_dir = project_dir / skills_dir_clean
        skills = _get_available_skills(str(full_skills_dir))
        print(f"✓ Found {len(skills)} skills in {skills_dir}")

        created_files = [str(mcp_config_path)]

        # 6. Create .cursor/rules/skilllite.mdc (only for project-level)
        if not is_global:
            rules_dir = cursor_dir / "rules"
            rules_dir.mkdir(parents=True, exist_ok=True)

            rules_path = rules_dir / "skilllite.mdc"
            rules_content = _generate_cursor_rules_content(skills)
            rules_path.write_text(rules_content, encoding="utf-8")
            print("✓ Created .cursor/rules/skilllite.mdc")
            created_files.append(str(rules_path))

        # 7. Summary
        print()
        print("=" * 50)
        print(f"🎉 SkillLite integration for Cursor initialized! ({mode_label})")
        print()
        print("Created files:")
        for f in created_files:
            print(f"  • {f}")
        print()
        if is_global:
            print(f"   SKILLLITE_SKILLS_DIR = {skills_dir_for_config}")
            print()
        print("Available MCP tools in Cursor:")
        print("  • skilllite_execute_code - Execute code in sandbox")
        print("  • skilllite_run_skill    - Run pre-defined skills")
        print("  • skilllite_list_skills  - List available skills")
        print("  • skilllite_get_skill_info - Get skill details")
        print("  • skilllite_scan_code    - Scan code for security issues")
        print()
        print("Next steps:")
        print("  1. Reload Cursor window (Cmd+Shift+P → Reload Window)")
        print("  2. Check MCP status in Cursor Settings → MCP")
        print("  3. Start chatting with Cursor Agent to use SkillLite tools")
        print("=" * 50)

        return 0
    except Exception as e:
        import traceback
        print(f"Error: {e}", file=sys.stderr)
        traceback.print_exc()
        return 1
