"""
Main entry point for skilllite CLI.

Provides the argument parser and main function.
"""

import argparse
import sys
from typing import Any, List, Optional

from ..sandbox.core import BINARY_VERSION
from .commands import (
    cmd_add, cmd_quickstart, cmd_list, cmd_remove, cmd_show, cmd_reindex,
    cmd_init_cursor, cmd_init_opencode, cmd_chat, cmd_mcp_server,
    cmd_install, cmd_uninstall, cmd_status, cmd_version, cmd_init,
)


def _add_args(parser: argparse.ArgumentParser, specs: List[Any]) -> None:
    """Add arguments from spec list.
    Spec: (pos, help?) | (long, short?, dest, default, help) | (long, short?, dest|'store_true', help) for flag.
    """
    for s in specs:
        if len(s) in (1, 2) and not s[0].startswith("-"):
            parser.add_argument(s[0], help=s[1] if len(s) == 2 else None)
        elif len(s) == 5:
            if s[3] == "store_true":
                kw = {"action": "store_true", "help": s[4]}
                if s[2]:
                    kw["dest"] = s[2]
                parser.add_argument(s[0], *(s[1],) if s[1] else (), **kw)
            else:
                parser.add_argument(s[0], *(s[1],) if s[1] else (), dest=s[2], default=s[3], help=s[4])
        elif len(s) == 4:
            parser.add_argument(s[0], *(s[1],) if s[1] else (), action="store_true", help=s[3])


# Config-driven subcommands: (name, help, func, arg_specs)
# Arg spec: (pos,) | (long, short, dest, default, help) | (long, short, "store_true", help)
_SUBCOMMANDS = [
    ("quickstart", "Zero-config quickstart: auto-detect LLM, install binary, launch chat", cmd_quickstart, [
        ("--skills-dir", "-s", "skills_dir", ".skills", "Skills directory path (default: .skills)"),
        ("--skills-repo", None, "skills_repo", None, "Remote skills repo (e.g. owner/repo, default: auto)"),
    ]),
    ("install", "Download and install the skillbox sandbox binary", cmd_install, [
        ("--version", None, "version", None, f"Version to install (default: {BINARY_VERSION})"),
        ("--force", "-f", "store_true", "Force reinstall even if already installed"),
        ("--quiet", "-q", "store_true", "Suppress progress output"),
    ]),
    ("uninstall", "Remove the installed skillbox binary", cmd_uninstall, []),
    ("status", "Show installation status", cmd_status, []),
    ("version", "Show version information", cmd_version, []),
    ("mcp", "Start MCP server (forwards to skillbox mcp)", cmd_mcp_server, [
        ("--skills-dir", "-s", "skills_dir", ".skills", "Skills directory path (default: .skills)"),
    ]),
    ("init-opencode", "Initialize SkillLite integration for OpenCode", cmd_init_opencode, [
        ("--project-dir", "-p", "project_dir", None, "Project directory (default: current directory)"),
        ("--skills-dir", "-s", "skills_dir", "./.skills", "Skills directory path (default: ./.skills)"),
        ("--force", "-f", "store_true", "Force overwrite existing opencode.json"),
    ]),
    ("init-cursor", "Initialize SkillLite integration for Cursor IDE", cmd_init_cursor, [
        ("--project-dir", "-p", "project_dir", None, "Project directory (default: current directory)"),
        ("--skills-dir", "-s", "skills_dir", "./.skills", "Skills directory path (default: ./.skills)"),
        ("--global", "-g", "global_mode", "store_true", "Install globally to ~/.cursor/mcp.json"),
        ("--force", "-f", None, "store_true", "Force overwrite existing .cursor/mcp.json"),
    ]),
    ("init", "Initialize SkillLite project (install binary, create .skills, install deps)", cmd_init, [
        ("--project-dir", "-p", "project_dir", None, "Project directory (default: current directory)"),
        ("--skills-dir", "-s", "skills_dir", ".skills", "Skills directory path (default: .skills)"),
        ("--skip-binary", None, "store_true", "Skip binary installation"),
        ("--skip-deps", None, "store_true", "Skip dependency installation"),
        ("--force", "-f", None, "store_true", "Force overwrite existing files"),
        ("--skip-audit", None, "store_true", "Skip dependency security audit (pip-audit, npm audit)"),
        ("--strict", None, "store_true", "Fail init if dependency audit finds known vulnerabilities"),
        ("--allow-unknown-packages", None, "store_true", "Allow packages from .skilllite.lock not in whitelist"),
        ("--use-llm", None, "use_llm", "store_true", "Use LLM to resolve dependencies (requires API key)"),
    ]),
    ("add", "Add skills from a remote repository or local path", cmd_add, [
        ("source", "Skill source (owner/repo, URL, or local path)"),
        ("--skills-dir", "-s", "skills_dir", ".skills", "Skills directory path (default: .skills)"),
        ("--force", "-f", "store_true", "Force overwrite existing skills"),
        ("--list", "-l", "store_true", "List available skills without installing"),
    ]),
    ("list", "List installed skills", cmd_list, [
        ("--skills-dir", "-s", "skills_dir", ".skills", "Skills directory path (default: .skills)"),
        ("--json", None, None, "store_true", "Output as JSON"),
    ]),
    ("show", "Show detailed information about a skill", cmd_show, [
        ("skill_name", "Name of the skill to show"),
        ("--skills-dir", "-s", "skills_dir", ".skills", "Skills directory path (default: .skills)"),
        ("--json", None, None, "store_true", "Output as JSON"),
    ]),
    ("reindex", "Rescan skills directory and rebuild metadata cache (delegates to skillbox)", cmd_reindex, [
        ("--skills-dir", "-s", "skills_dir", ".skills", "Skills directory path (default: .skills)"),
        ("--verbose", "-v", "store_true", "Verbose output"),
    ]),
    ("chat", "Interactive chat with persistent transcript and memory", cmd_chat, [
        ("--workspace", "-w", "workspace", None, "Workspace path for chat data (default: ~/.skilllite/chat)"),
        ("--session", "-s", "session", "main", "Session key (default: main)"),
        ("--skills-dir", None, "skills_dir", ".skills", "Skills directory (default: .skills)"),
        ("--quiet", "-q", "store_true", "Suppress verbose output"),
    ]),
    ("remove", "Remove an installed skill", cmd_remove, [
        ("skill_name", "Name of the skill to remove"),
        ("--skills-dir", "-s", "skills_dir", ".skills", "Skills directory path (default: .skills)"),
        ("--force", "-f", "store_true", "Skip confirmation prompt"),
    ]),
]


def create_parser() -> argparse.ArgumentParser:
    """Create the argument parser."""
    parser = argparse.ArgumentParser(
        prog="skilllite",
        description="SkillLite - A lightweight Skills execution engine with LLM integration",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  skilllite quickstart       Zero-config quickstart (recommended for first use!)
  skilllite install          Install the sandbox binary
  skilllite install --force  Force reinstall
  skilllite status           Check installation status
  skilllite uninstall        Remove the binary
  skilllite mcp              Start MCP server (requires pip install skilllite[mcp])
  skilllite init             Initialize SkillLite project (binary + .skills + deps)
  skilllite init-opencode    Initialize OpenCode integration
  skilllite init-cursor      Initialize Cursor IDE integration
  skilllite add owner/repo   Add skills from a remote repository
  skilllite list             List installed skills
  skilllite remove <name>    Remove an installed skill
  skilllite reindex          Rescan skills directory and rebuild metadata cache
  skilllite chat             Interactive chat (requires skillbox --features executor)

For more information, visit: https://github.com/skilllite/skilllite
        """
    )
    parser.add_argument("-V", "--version", action="store_true", help="Show version information")
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    for name, help_text, func, specs in _SUBCOMMANDS:
        p = subparsers.add_parser(name, help=help_text)
        _add_args(p, specs)
        p.set_defaults(func=func)

    return parser


def main(argv: Optional[List[str]] = None) -> int:
    """Main entry point for the CLI."""
    parser = create_parser()
    args = parser.parse_args(argv)

    if args.version:
        return cmd_version(args)

    if not args.command:
        parser.print_help()
        return 0

    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
