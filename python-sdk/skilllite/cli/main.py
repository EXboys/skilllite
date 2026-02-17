"""
Main entry point for skilllite CLI.

Provides the argument parser and main function.
"""

import argparse
import sys
from typing import List, Optional

from ..sandbox.core import BINARY_VERSION
from .binary import cmd_install, cmd_uninstall, cmd_status, cmd_version
from .commands import (
    cmd_add, cmd_quickstart, cmd_list, cmd_remove, cmd_show, cmd_reindex,
    cmd_init_cursor, cmd_init_opencode, cmd_chat, cmd_mcp_server,
)
from .init import cmd_init


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

    parser.add_argument(
        "-V", "--version",
        action="store_true",
        help="Show version information"
    )

    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # quickstart command (recommended for first use)
    quickstart_parser = subparsers.add_parser(
        "quickstart",
        help="Zero-config quickstart: auto-detect LLM, install binary, launch chat"
    )
    quickstart_parser.add_argument(
        "--skills-dir", "-s",
        dest="skills_dir",
        default=".skills",
        help="Skills directory path (default: .skills)"
    )
    quickstart_parser.add_argument(
        "--skills-repo",
        dest="skills_repo",
        default=None,
        help="Remote skills repo to download from (e.g. owner/repo, default: auto)"
    )
    quickstart_parser.set_defaults(func=cmd_quickstart)

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
        help="Start MCP server (forwards to skillbox mcp)"
    )
    mcp_parser.add_argument(
        "--skills-dir", "-s",
        dest="skills_dir",
        default=".skills",
        help="Skills directory path (default: .skills)"
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

    # init-cursor command
    init_cursor_parser = subparsers.add_parser(
        "init-cursor",
        help="Initialize SkillLite integration for Cursor IDE"
    )
    init_cursor_parser.add_argument(
        "--project-dir", "-p",
        dest="project_dir",
        default=None,
        help="Project directory (default: current directory)"
    )
    init_cursor_parser.add_argument(
        "--skills-dir", "-s",
        dest="skills_dir",
        default="./.skills",
        help="Skills directory path (default: ./.skills)"
    )
    init_cursor_parser.add_argument(
        "--global", "-g",
        dest="global_mode",
        action="store_true",
        help="Install globally to ~/.cursor/mcp.json (available in all Cursor projects)"
    )
    init_cursor_parser.add_argument(
        "--force", "-f",
        action="store_true",
        help="Force overwrite existing .cursor/mcp.json"
    )
    init_cursor_parser.set_defaults(func=cmd_init_cursor)

    # init command
    init_parser = subparsers.add_parser(
        "init",
        help="Initialize SkillLite project (install binary, create .skills, install deps)"
    )
    init_parser.add_argument(
        "--project-dir", "-p",
        dest="project_dir",
        default=None,
        help="Project directory (default: current directory)"
    )
    init_parser.add_argument(
        "--skills-dir", "-s",
        dest="skills_dir",
        default=".skills",
        help="Skills directory path (default: .skills)"
    )
    init_parser.add_argument(
        "--skip-binary",
        action="store_true",
        help="Skip binary installation"
    )
    init_parser.add_argument(
        "--skip-deps",
        action="store_true",
        help="Skip dependency installation"
    )
    init_parser.add_argument(
        "--force", "-f",
        action="store_true",
        help="Force overwrite existing files"
    )
    init_parser.add_argument(
        "--skip-audit",
        action="store_true",
        help="Skip dependency security audit (pip-audit, npm audit)"
    )
    init_parser.add_argument(
        "--strict",
        action="store_true",
        help="Fail init if dependency audit finds known vulnerabilities"
    )
    init_parser.add_argument(
        "--allow-unknown-packages",
        action="store_true",
        help="Allow packages from .skilllite.lock that are not in the security whitelist"
    )
    init_parser.add_argument(
        "--use-llm",
        dest="use_llm",
        action="store_true",
        help="Use LLM to resolve dependencies from compatibility string (requires API key)"
    )
    init_parser.set_defaults(func=cmd_init)

    # add command
    add_parser = subparsers.add_parser(
        "add",
        help="Add skills from a remote repository or local path"
    )
    add_parser.add_argument(
        "source",
        help="Skill source (owner/repo, URL, or local path)"
    )
    add_parser.add_argument(
        "--skills-dir", "-s",
        dest="skills_dir",
        default=".skills",
        help="Skills directory path (default: .skills)"
    )
    add_parser.add_argument(
        "--force", "-f",
        action="store_true",
        help="Force overwrite existing skills"
    )
    add_parser.add_argument(
        "--list", "-l",
        action="store_true",
        help="List available skills without installing"
    )
    add_parser.set_defaults(func=cmd_add)

    # list command
    list_parser = subparsers.add_parser(
        "list",
        help="List installed skills"
    )
    list_parser.add_argument(
        "--skills-dir", "-s",
        dest="skills_dir",
        default=".skills",
        help="Skills directory path (default: .skills)"
    )
    list_parser.add_argument(
        "--json",
        action="store_true",
        help="Output as JSON"
    )
    list_parser.set_defaults(func=cmd_list)

    # show command
    show_parser = subparsers.add_parser(
        "show",
        help="Show detailed information about a skill"
    )
    show_parser.add_argument(
        "skill_name",
        help="Name of the skill to show"
    )
    show_parser.add_argument(
        "--skills-dir", "-s",
        dest="skills_dir",
        default=".skills",
        help="Skills directory path (default: .skills)"
    )
    show_parser.add_argument(
        "--json",
        action="store_true",
        help="Output as JSON"
    )
    show_parser.set_defaults(func=cmd_show)

    # reindex command
    reindex_parser = subparsers.add_parser(
        "reindex",
        help="Rescan skills directory and rebuild metadata cache (delegates to skillbox)"
    )
    reindex_parser.add_argument(
        "--skills-dir", "-s",
        dest="skills_dir",
        default=".skills",
        help="Skills directory path (default: .skills)"
    )
    reindex_parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Verbose output"
    )
    reindex_parser.set_defaults(func=cmd_reindex)

    # chat command
    chat_parser = subparsers.add_parser(
        "chat",
        help="Interactive chat with persistent transcript and memory"
    )
    chat_parser.add_argument(
        "--workspace", "-w",
        dest="workspace",
        default=None,
        help="Workspace path for chat data (default: ~/.skilllite/chat)"
    )
    chat_parser.add_argument(
        "--session", "-s",
        dest="session",
        default="main",
        help="Session key (default: main)"
    )
    chat_parser.add_argument(
        "--skills-dir",
        dest="skills_dir",
        default=".skills",
        help="Skills directory (default: .skills)"
    )
    chat_parser.add_argument(
        "--quiet", "-q",
        action="store_true",
        help="Suppress verbose output"
    )
    chat_parser.set_defaults(func=cmd_chat)

    # remove command
    remove_parser = subparsers.add_parser(
        "remove",
        help="Remove an installed skill"
    )
    remove_parser.add_argument(
        "skill_name",
        help="Name of the skill to remove"
    )
    remove_parser.add_argument(
        "--skills-dir", "-s",
        dest="skills_dir",
        default=".skills",
        help="Skills directory path (default: .skills)"
    )
    remove_parser.add_argument(
        "--force", "-f",
        action="store_true",
        help="Skip confirmation prompt"
    )
    remove_parser.set_defaults(func=cmd_remove)

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

