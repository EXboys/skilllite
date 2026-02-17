#!/usr/bin/env python3
"""
Verify SkillLite MCP setup for OpenCode integration.

This script checks that all prerequisites are met for using
SkillLite as an MCP server with OpenCode.

Usage:
    python verify_setup.py
"""

import os
import sys
import json
import shutil
import subprocess
from pathlib import Path

def check_mark(success: bool) -> str:
    return "✅" if success else "❌"

def check_skilllite_installed() -> bool:
    """Check if skilllite is installed."""
    try:
        import skilllite
        return True
    except ImportError:
        return False

def check_mcp_installed() -> bool:
    """Check if MCP library is installed."""
    try:
        import mcp
        return True
    except ImportError:
        return False

def check_skillbox_installed() -> bool:
    """Check if skillbox binary is installed."""
    try:
        from skilllite.sandbox.skillbox import find_binary, is_installed
        return is_installed()
    except Exception:
        return False

def check_opencode_installed() -> bool:
    """Check if OpenCode is installed."""
    return shutil.which("opencode") is not None

def get_opencode_config_path() -> Path:
    """Get OpenCode config file path."""
    # Check project-level first
    project_config = Path(".opencode/config.json")
    if project_config.exists():
        return project_config
    
    # Check global config
    home = Path.home()
    global_config = home / ".config" / "opencode" / "config.json"
    return global_config

def check_opencode_config() -> tuple[bool, str]:
    """Check if OpenCode is configured with SkillLite MCP."""
    config_path = get_opencode_config_path()
    
    if not config_path.exists():
        return False, f"Config not found at {config_path}"
    
    try:
        with open(config_path) as f:
            config = json.load(f)
        
        mcp_servers = config.get("mcp", {}).get("servers", {})
        if "skilllite" in mcp_servers:
            return True, f"Found in {config_path}"
        else:
            return False, f"'skilllite' not in mcp.servers at {config_path}"
    except Exception as e:
        return False, f"Error reading config: {e}"

def test_mcp_server() -> tuple[bool, str]:
    """Test if MCP server can start (skilllite mcp forwards to Rust binary)."""
    try:
        result = subprocess.run(
            ["skilllite", "mcp", "--help"],
            capture_output=True,
            text=True,
            timeout=5
        )
        if result.returncode == 0:
            return True, "MCP server command available"
        return False, result.stderr or "skilllite mcp not found (run: skilllite install)"
    except FileNotFoundError:
        return False, "skilllite not in PATH (run: skilllite install)"
    except subprocess.TimeoutExpired:
        return False, "Timeout starting MCP server"
    except Exception as e:
        return False, str(e)

def generate_config() -> str:
    """Generate sample OpenCode config."""
    config = {
        "mcp": {
            "servers": {
                "skilllite": {
                    "command": "skilllite",
                    "args": ["mcp"],
                    "env": {
                        "SKILLBOX_SANDBOX_LEVEL": "3"
                    }
                }
            }
        }
    }
    return json.dumps(config, indent=2)

def main():
    print("=" * 60)
    print("🔍 SkillLite + OpenCode Integration Verification")
    print("=" * 60)
    print()
    
    # Check prerequisites
    print("📦 Prerequisites:")
    
    skilllite_ok = check_skilllite_installed()
    print(f"  {check_mark(skilllite_ok)} SkillLite installed")
    
    mcp_ok = check_mcp_installed()
    print(f"  {check_mark(mcp_ok)} MCP library installed")
    if not mcp_ok:
        print("     → Run: pip install skilllite[mcp]")
    
    skillbox_ok = check_skillbox_installed()
    print(f"  {check_mark(skillbox_ok)} Skillbox binary installed")
    if not skillbox_ok:
        print("     → Run: skilllite install")
    
    opencode_ok = check_opencode_installed()
    print(f"  {check_mark(opencode_ok)} OpenCode installed")
    if not opencode_ok:
        print("     → Run: brew install anomalyco/tap/opencode")
    
    print()
    print("⚙️  Configuration:")
    
    config_ok, config_msg = check_opencode_config()
    print(f"  {check_mark(config_ok)} OpenCode MCP config")
    print(f"     {config_msg}")
    
    if not config_ok:
        print()
        print("📝 Sample configuration for .opencode/config.json:")
        print("-" * 40)
        print(generate_config())
        print("-" * 40)
    
    print()
    print("🧪 Functionality Test:")
    
    if skilllite_ok and mcp_ok:
        server_ok, server_msg = test_mcp_server()
        print(f"  {check_mark(server_ok)} MCP Server initialization")
        if not server_ok:
            print(f"     Error: {server_msg}")
    else:
        print("  ⏭️  Skipped (missing dependencies)")
    
    print()
    all_ok = skilllite_ok and mcp_ok and skillbox_ok and opencode_ok and config_ok
    if all_ok:
        print("✅ All checks passed! You can now use SkillLite with OpenCode.")
        print()
        print("🚀 Start OpenCode:")
        print("   opencode")
    else:
        print("⚠️  Some checks failed. Please fix the issues above.")
    
    return 0 if all_ok else 1

if __name__ == "__main__":
    sys.exit(main())

