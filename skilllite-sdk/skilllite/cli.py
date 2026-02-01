"""
Command-line interface for skilllite.

Provides commands for managing the skillbox binary, similar to
how Playwright provides `playwright install` for browser management.

Usage:
    skilllite install       # Download and install the sandbox binary
    skilllite uninstall     # Remove the installed binary
    skilllite status        # Show installation status
    skilllite version       # Show version information
    skilllite mcp           # Start MCP server
    skilllite init-opencode # Initialize OpenCode integration
"""

import argparse
import json
import os
import sys
from pathlib import Path
from typing import List, Optional, Dict, Any

from . import __version__
from .sandbox.skillbox import (
    BINARY_VERSION,
    get_platform,
    install,
    is_installed,
    get_installed_version,
    uninstall,
)


def print_status() -> None:
    """Print installation status."""
    from .sandbox.skillbox import find_binary, get_binary_path
    
    print("SkillLite Installation Status")
    print("=" * 40)
    
    if is_installed():
        version = get_installed_version()
        print(f"✓ skillbox is installed (v{version})")
        print(f"  Location: {get_binary_path()}")
    else:
        binary = find_binary()
        if binary:
            print(f"✓ skillbox found at: {binary}")
        else:
            print("✗ skillbox is not installed")
            print("  Install with: skilllite install")

def cmd_install(args: argparse.Namespace) -> int:
    """Install the skillbox binary."""
    try:
        install(
            version=args.version,
            force=args.force,
            show_progress=not args.quiet
        )
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

def cmd_uninstall(args: argparse.Namespace) -> int:
    """Uninstall the skillbox binary."""
    try:
        uninstall()
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

def cmd_status(args: argparse.Namespace) -> int:
    """Show installation status."""
    try:
        print_status()
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1

def cmd_version(args: argparse.Namespace) -> int:
    """Show version information."""
    print(f"skilllite Python SDK: v{__version__}")
    print(f"skillbox binary (bundled): v{BINARY_VERSION}")
    
    installed_version = get_installed_version()
    if installed_version:
        print(f"skillbox binary (installed): v{installed_version}")
    else:
        print("skillbox binary (installed): not installed")
    
    try:
        plat = get_platform()
        print(f"Platform: {plat}")
    except RuntimeError as e:
        print(f"Platform: {e}")
    
    return 0

def cmd_mcp_server(args: argparse.Namespace) -> int:
    """Start MCP server."""
    try:
        import asyncio
        from .mcp.server import main as mcp_main

        asyncio.run(mcp_main())
        return 0
    except ImportError as e:
        print("Error: MCP integration not available", file=sys.stderr)
        print("Please install it with: pip install skilllite[mcp]", file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        print("\nMCP server stopped by user", file=sys.stderr)
        return 0
    except Exception as e:
        import traceback
        print(f"Error starting MCP server: {e}", file=sys.stderr)
        traceback.print_exc()
        return 1


def _get_available_skills(skills_dir: str) -> List[Dict[str, Any]]:
    """Get list of available skills from the skills directory."""
    skills = []
    skills_path = Path(skills_dir)

    if not skills_path.exists():
        return skills

    for skill_dir in skills_path.iterdir():
        if not skill_dir.is_dir():
            continue

        skill_md = skill_dir / "SKILL.md"
        if not skill_md.exists():
            continue

        # Parse basic skill info from SKILL.md
        try:
            content = skill_md.read_text()
            name = skill_dir.name
            description = ""

            # Extract description from YAML frontmatter
            if content.startswith("---"):
                parts = content.split("---", 2)
                if len(parts) >= 3:
                    import re
                    desc_match = re.search(r'description:\s*["\']?([^"\'\n]+)', parts[1])
                    if desc_match:
                        description = desc_match.group(1).strip()

            skills.append({
                "name": name,
                "description": description or f"Execute {name} skill"
            })
        except Exception:
            continue

    return skills


def _generate_skill_md_content(skills: List[Dict[str, Any]]) -> str:
    """Generate the SKILL.md content for OpenCode integration."""
    skills_list = ""
    if skills:
        for skill in skills:
            skills_list += f"- **{skill['name']}**: {skill['description']}\n"
    else:
        skills_list = "- (No pre-defined skills found. Use skilllite_execute_code for code execution.)\n"

    return f'''---
name: skilllite
description: 在安全沙箱中执行代码或预定义技能。当需要运行不信任的代码、网络请求、数据处理时使用。
---

## 概述

SkillLite 提供了一个安全的沙箱执行环境。代码在系统级沙箱（macOS Seatbelt / Linux Namespace）中隔离运行，防止恶意代码影响主机系统。

## 何时使用 SkillLite 而不是 bash

| 场景 | 用 bash | 用 SkillLite |
|-----|---------|-------------|
| git 操作 | ✅ | |
| 读取项目文件 | ✅ | |
| 执行用户提供的代码 | | ✅ |
| 网络请求/API 调用 | | ✅ |
| 数据分析处理 | | ✅ |
| 运行不信任的脚本 | | ✅ |
| 执行可能危险的命令 | | ✅ |

## 可用工具

### 1. skilllite_execute_code
在沙箱中执行任意代码（Python/JavaScript/Bash）。

**参数：**
- `language`: "python" | "javascript" | "bash"
- `code`: 要执行的代码
- `confirmed`: 是否确认执行（高危代码需要）
- `scan_id`: 扫描 ID（确认执行时需要）

**安全确认流程：**
当检测到危险代码时，会返回安全报告和 `scan_id`。向用户展示安全问题后，如果用户同意执行，需要再次调用时设置 `confirmed=true` 和返回的 `scan_id`。

### 2. skilllite_run_skill
执行预定义技能。

**参数：**
- `skill_name`: 技能名称
- `input`: 技能的输入参数（JSON 对象）

### 3. skilllite_list_skills
查看所有可用的预定义技能。无需参数。

### 4. skilllite_get_skill_info
获取指定技能的详细信息，包括输入参数模式。

**参数：**
- `skill_name`: 技能名称

### 5. skilllite_scan_code
仅扫描代码安全性，不执行。用于预检查代码是否安全。

**参数：**
- `language`: "python" | "javascript" | "bash"
- `code`: 要扫描的代码

## 预定义技能

{skills_list}

## 使用示例

### 执行 Python 代码
```
skilllite_execute_code(language="python", code="print(sum(range(1, 101)))")
```

### 处理危险代码
1. 调用 `skilllite_execute_code` 执行代码
2. 如果返回 `requires_confirmation=true`，向用户展示安全问题
3. 用户确认后，再次调用时带上 `confirmed=true` 和 `scan_id`

### 使用预定义技能
```
skilllite_list_skills()  # 查看可用技能
skilllite_get_skill_info(skill_name="calculator")  # 查看技能参数
skilllite_run_skill(skill_name="calculator", input={{"operation": "add", "a": 5, "b": 3}})
```
'''


def _detect_best_command() -> tuple[List[str], str]:
    """
    Detect the best command to start the MCP server.

    Returns:
        Tuple of (command_list, description)

    Priority:
    1. uvx (if available) - most portable, auto-manages environment
    2. pipx (if available) - similar to uvx
    3. python3 -m skilllite.mcp.server - if skilllite is in PATH's python
    4. Full python path - fallback
    """
    import shutil

    # Check if uvx is available
    if shutil.which("uvx"):
        return (["uvx", "skilllite", "mcp"], "uvx (auto-managed)")

    # Check if pipx is available and skilllite is installed via pipx
    if shutil.which("pipx"):
        # Check if skilllite is installed in pipx
        try:
            import subprocess
            result = subprocess.run(
                ["pipx", "list", "--short"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if "skilllite" in result.stdout:
                return (["pipx", "run", "skilllite", "mcp"], "pipx (installed)")
        except Exception:
            pass

    # Check if skilllite command is directly available in PATH
    if shutil.which("skilllite"):
        return (["skilllite", "mcp"], "skilllite (in PATH)")

    # Check if python3 has skilllite installed
    python3_path = shutil.which("python3")
    if python3_path:
        try:
            import subprocess
            result = subprocess.run(
                [python3_path, "-c", "import skilllite; print('ok')"],
                capture_output=True,
                text=True,
                timeout=5
            )
            if result.returncode == 0 and "ok" in result.stdout:
                return (["python3", "-m", "skilllite.mcp.server"], "python3 (skilllite installed)")
        except Exception:
            pass

    # Fallback: use current Python's full path
    return ([sys.executable, "-m", "skilllite.mcp.server"], "full path (fallback)")


def _generate_opencode_config(command: List[str], skills_dir: str) -> Dict[str, Any]:
    """Generate OpenCode configuration."""
    return {
        "$schema": "https://opencode.ai/config.json",
        "mcp": {
            "skilllite": {
                "type": "local",
                "command": command,
                "environment": {
                    "SKILLBOX_SANDBOX_LEVEL": "3",
                    "SKILLLITE_SKILLS_DIR": skills_dir
                },
                "enabled": True
            }
        }
    }


def cmd_init_opencode(args: argparse.Namespace) -> int:
    """Initialize OpenCode integration."""
    try:
        project_dir = Path(args.project_dir or os.getcwd())
        skills_dir = args.skills_dir or "./.skills"

        print("🚀 Initializing SkillLite integration for OpenCode...")
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

        # 3. Create opencode.json
        opencode_config_path = project_dir / "opencode.json"
        config = _generate_opencode_config(command, skills_dir)

        if opencode_config_path.exists() and not args.force:
            # Merge with existing config
            try:
                existing = json.loads(opencode_config_path.read_text())
                if "mcp" not in existing:
                    existing["mcp"] = {}
                existing["mcp"]["skilllite"] = config["mcp"]["skilllite"]
                if "$schema" not in existing:
                    existing["$schema"] = config["$schema"]
                config = existing
                print(f"✓ Updated existing opencode.json")
            except Exception:
                print(f"⚠ Could not parse existing opencode.json, overwriting")
        else:
            print(f"✓ Created opencode.json")

        opencode_config_path.write_text(json.dumps(config, indent=2, ensure_ascii=False))

        # 4. Get available skills
        # Handle relative path properly - remove leading "./" but keep the rest
        skills_dir_clean = skills_dir[2:] if skills_dir.startswith("./") else skills_dir
        full_skills_dir = project_dir / skills_dir_clean
        skills = _get_available_skills(str(full_skills_dir))
        print(f"✓ Found {len(skills)} skills in {skills_dir}")

        # 5. Create .opencode/skills/skilllite/SKILL.md
        skill_dir = project_dir / ".opencode" / "skills" / "skilllite"
        skill_dir.mkdir(parents=True, exist_ok=True)

        skill_md_path = skill_dir / "SKILL.md"
        skill_md_content = _generate_skill_md_content(skills)
        skill_md_path.write_text(skill_md_content, encoding="utf-8")
        print(f"✓ Created .opencode/skills/skilllite/SKILL.md")

        # 6. Summary
        print()
        print("=" * 50)
        print("🎉 SkillLite integration initialized successfully!")
        print()
        print("Created files:")
        print(f"  • {opencode_config_path.relative_to(project_dir)}")
        print(f"  • {skill_md_path.relative_to(project_dir)}")
        print()
        print("Available MCP tools in OpenCode:")
        print("  • skilllite_execute_code - Execute code in sandbox")
        print("  • skilllite_run_skill    - Run pre-defined skills")
        print("  • skilllite_list_skills  - List available skills")
        print("  • skilllite_get_skill_info - Get skill details")
        print("  • skilllite_scan_code    - Scan code for security issues")
        print()
        print("Start OpenCode with: opencode")
        print("=" * 50)

        return 0
    except Exception as e:
        import traceback
        print(f"Error: {e}", file=sys.stderr)
        traceback.print_exc()
        return 1

def create_parser() -> argparse.ArgumentParser:
    """Create the argument parser."""
    parser = argparse.ArgumentParser(
        prog="skilllite",
        description="SkillLite - A lightweight Skills execution engine with LLM integration",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  skilllite install          Install the sandbox binary
  skilllite install --force  Force reinstall
  skilllite status           Check installation status
  skilllite uninstall        Remove the binary
  skilllite mcp              Start MCP server (requires pip install skilllite[mcp])
  skilllite init-opencode    Initialize OpenCode integration

For more information, visit: https://github.com/skilllite/skilllite
        """
    )

    parser.add_argument(
        "-V", "--version",
        action="store_true",
        help="Show version information"
    )

    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # install command
    install_parser = subparsers.add_parser(
        "install",
        help="Download and install the skillbox sandbox binary"
    )
    install_parser.add_argument(
        "--version",
        dest="version",
        default=None,
        help=f"Version to install (default: {BINARY_VERSION})"
    )
    install_parser.add_argument(
        "--force", "-f",
        action="store_true",
        help="Force reinstall even if already installed"
    )
    install_parser.add_argument(
        "--quiet", "-q",
        action="store_true",
        help="Suppress progress output"
    )
    install_parser.set_defaults(func=cmd_install)

    # uninstall command
    uninstall_parser = subparsers.add_parser(
        "uninstall",
        help="Remove the installed skillbox binary"
    )
    uninstall_parser.set_defaults(func=cmd_uninstall)

    # status command
    status_parser = subparsers.add_parser(
        "status",
        help="Show installation status"
    )
    status_parser.set_defaults(func=cmd_status)

    # version command (alternative to -V)
    version_parser = subparsers.add_parser(
        "version",
        help="Show version information"
    )
    version_parser.set_defaults(func=cmd_version)

    # mcp command
    mcp_parser = subparsers.add_parser(
        "mcp",
        help="Start MCP server for SkillLite"
    )
    mcp_parser.set_defaults(func=cmd_mcp_server)

    # init-opencode command
    init_opencode_parser = subparsers.add_parser(
        "init-opencode",
        help="Initialize SkillLite integration for OpenCode"
    )
    init_opencode_parser.add_argument(
        "--project-dir", "-p",
        dest="project_dir",
        default=None,
        help="Project directory (default: current directory)"
    )
    init_opencode_parser.add_argument(
        "--skills-dir", "-s",
        dest="skills_dir",
        default="./.skills",
        help="Skills directory path (default: ./.skills)"
    )
    init_opencode_parser.add_argument(
        "--force", "-f",
        action="store_true",
        help="Force overwrite existing opencode.json"
    )
    init_opencode_parser.set_defaults(func=cmd_init_opencode)

    return parser

def main(argv: Optional[List[str]] = None) -> int:
    """Main entry point for the CLI."""
    parser = create_parser()
    args = parser.parse_args(argv)
    
    # Handle -V/--version flag
    if args.version:
        return cmd_version(args)
    
    # Handle no command
    if not args.command:
        parser.print_help()
        return 0
    
    # Execute the command
    return args.func(args)

if __name__ == "__main__":
    sys.exit(main())
